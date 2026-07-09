<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import type { Diagnostic } from "@codemirror/lint";
	import {
		applyYaml,
		cancelBackendRequests,
		getDynamicResourceDetails,
		getResourceYaml,
		isAppError,
		lintKubernetesYaml,
		prepareYamlApply,
		type TauriClient,
	} from "@/lib/tauri";
	import { createCancellableRequest, createCancelScope } from "@/lib/cancellable-loads";
	import { diagnosticLog, diagnosticResultSummary } from "@/lib/diagnostics";
	import { withForegroundLoad } from "@/lib/foreground-loading";
	import { queryKeys } from "@/lib/queryKeys";
	import { getSettingsSnapshot, settingsStore } from "@/lib/settings-store";
	import type {
		DiscoveredResourceKind,
		KubernetesYamlLintDiagnostic,
		KubernetesYamlLintStatusNote,
		ResourceSummary,
		YamlApplyPreview,
		YamlEncoding,
		YamlViewMode,
	} from "@/lib/types";
	import { formatYamlDocument } from "@/lib/yamlFormat";
	import { getErrorMessage } from "./helpers";
	import YamlTab from "./YamlTab.svelte";
	import {
		buildYamlDryRunDiff,
		findYamlFieldRange,
	} from "./yamlTabDiff";
	import {
		buildYamlApplyRequest as createYamlApplyRequest,
		isYamlApplyDisabled,
		resolveYamlForceConflicts,
		yamlAppliedMessage as formatYamlAppliedMessage,
		yamlApplyTargetLabel,
	} from "./yamlApplyModel";

	let {
		client,
		resource,
		dynamicKind,
		kubeconfigSourceKey,
		detailsYaml = "",
		detailsQueryKey,
		detailsEnabled,
		active,
		refreshVersion,
		yamlViewMode = $bindable(getSettingsSnapshot().yamlViewModeDefault),
		yamlEncoding = $bindable(getSettingsSnapshot().yamlEncodingDefault),
		yamlShowFullDiff = $bindable(false),
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		dynamicKind: DiscoveredResourceKind | null;
		kubeconfigSourceKey?: string;
		detailsYaml?: string;
		detailsQueryKey: readonly unknown[];
		detailsEnabled: boolean;
		active: boolean;
		refreshVersion: number;
		yamlViewMode: YamlViewMode;
		yamlEncoding: YamlEncoding;
		yamlShowFullDiff: boolean;
	} = $props();

	const queryClient = useQueryClient();
	let yamlEditing = $state(false);
	let yamlDraft = $state("");
	let yamlLoadingDraft = $state(false);
	let yamlPreview = $state<YamlApplyPreview | null>(null);
	let yamlPreviewForceConflicts = $state(false);
	let yamlForceConflictsForResource = $state(false);
	let yamlLintDiagnostics = $state<KubernetesYamlLintDiagnostic[]>([]);
	let yamlLintNotes = $state<KubernetesYamlLintStatusNote[]>([]);
	let yamlLintError = $state("");
	let yamlPreparing = $state(false);
	let yamlApplying = $state(false);
	let yamlFormatError = $state("");
	let yamlPrepareRawError = $state<unknown>(null);
	let yamlPrepareError = $state("");
	let yamlApplyRawError = $state<unknown>(null);
	let yamlApplyError = $state("");
	let yamlAppliedMessage = $state("");
	const pendingCancelTimers = new Map<string, ReturnType<typeof setTimeout>>();

	const dynamicKindKey = $derived(
		dynamicKind
			? `${dynamicKind.group}/${dynamicKind.version}/${dynamicKind.kind}/${dynamicKind.plural}/${dynamicKind.namespaced}`
			: "",
	);
	const yamlQueryKey = $derived([
		...queryKeys.resourceYaml(
			resource,
			dynamicKindKey,
			kubeconfigSourceKey,
			yamlViewMode,
			yamlEncoding,
		),
		refreshVersion,
	]);
	const yamlCancelScope = $derived(createCancelScope("resource-yaml", yamlQueryKey));
	const yamlEnabled = $derived(detailsEnabled && active);
	const yamlApplyDisabledReason = $derived(isYamlApplyDisabled(resource));
	const yamlApplyTarget = $derived(yamlApplyTargetLabel(resource));
	const canAllowYamlForceConflicts = $derived(
		!$settingsStore.allowYamlForceConflicts &&
			!yamlForceConflictsForResource &&
			isAppError(yamlPrepareRawError) &&
			yamlPrepareRawError.kind === "fieldManagerConflict",
	);
	const yamlDiffLines = $derived(
		yamlPreview
			? buildYamlDryRunDiff({
					currentYaml: yamlPreview.currentYaml,
					dryRunYaml: yamlPreview.dryRunYaml,
					style: $settingsStore.yamlDiffStyle,
					full: yamlShowFullDiff,
					forceConflicts: yamlPreviewForceConflicts,
				})
			: [],
	);
	const visibleYamlDiffLines = $derived(
		yamlShowFullDiff ? yamlDiffLines : yamlDiffLines.slice(0, 24),
	);
	const hiddenYamlDiffCount = $derived(
		Math.max(0, yamlDiffLines.length - visibleYamlDiffLines.length),
	);

	async function runYamlFetch<T>(loadLabel: string, task: () => Promise<T>): Promise<T> {
		const started = performance.now();
		diagnosticLog("detail.yaml.fetch.start", { key: resourceKey() });
		const result = await withForegroundLoad(loadLabel, task);
		diagnosticLog("detail.yaml.fetch.done", {
			key: resourceKey(),
			ms: Math.round(performance.now() - started),
			result: diagnosticResultSummary(result),
		});
		return result;
	}

	function resourceKey(): string {
		return `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
	}

	function cancelPendingBackendScope(cancelScope: string) {
		const timer = pendingCancelTimers.get(cancelScope);
		if (!timer) return;
		clearTimeout(timer);
		pendingCancelTimers.delete(cancelScope);
	}

	function scheduleBackendScopeCancel(cancelScope: string, queryKey: readonly unknown[]) {
		cancelPendingBackendScope(cancelScope);
		const timer = setTimeout(() => {
			pendingCancelTimers.delete(cancelScope);
			void queryClient.cancelQueries({ queryKey, exact: true });
			void cancelBackendRequests(client, cancelScope).catch((error: unknown) => {
				diagnosticLog("detail.yaml.cancel.error", {
					error: error instanceof Error ? error.message : String(error),
				});
			});
		}, 0);
		pendingCancelTimers.set(cancelScope, timer);
	}

	$effect(() => {
		const currentYamlCancelScope = yamlCancelScope;
		const currentYamlQueryKey = yamlQueryKey;
		cancelPendingBackendScope(currentYamlCancelScope);
		return () => scheduleBackendScopeCancel(currentYamlCancelScope, currentYamlQueryKey);
	});

	const yamlQuery = createQuery<string>(() => ({
		queryKey: yamlQueryKey,
		queryFn: async () => {
			try {
				return await runYamlFetch("resource-yaml", async () => {
					if (dynamicKind) {
						return (
							detailsYaml ||
							(
								await getDynamicResourceDetails(
									client,
									resource.cluster,
									dynamicKind,
									resource.name,
									resource.namespace ?? undefined,
									kubeconfigSourceKey,
									yamlViewMode,
									yamlEncoding,
									createCancellableRequest(yamlCancelScope, "yaml"),
								)
							).yaml
						);
					}
					return await getResourceYaml(
						client,
						resource.cluster,
						resource.kind,
						resource.name,
						resource.namespace ?? undefined,
						kubeconfigSourceKey,
						yamlViewMode,
						yamlEncoding,
						createCancellableRequest(yamlCancelScope, "yaml"),
					);
				});
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("detail.yaml.cancel", { key: resourceKey() });
				}
				throw error;
			}
		},
		enabled: yamlEnabled,
		retry: false,
		staleTime: 30_000,
	}));
	const yamlText = $derived(yamlQuery.data ?? detailsYaml);

	$effect(() => {
		resource.cluster;
		resource.kind;
		resource.name;
		resource.namespace;
		resetYamlApply();
	});

	function setYamlViewMode(value: string) {
		yamlViewMode = value as YamlViewMode;
		resetYamlApply();
	}

	function setYamlEncoding(value: string) {
		yamlEncoding = value as YamlEncoding;
		resetYamlApply();
	}

	function resetYamlApply() {
		yamlEditing = false;
		yamlDraft = "";
		yamlLoadingDraft = false;
		yamlPreview = null;
		yamlPreviewForceConflicts = false;
		yamlForceConflictsForResource = false;
		yamlLintDiagnostics = [];
		yamlLintNotes = [];
		yamlLintError = "";
		yamlPreparing = false;
		yamlApplying = false;
		yamlFormatError = "";
		yamlPrepareRawError = null;
		yamlPrepareError = "";
		yamlApplyRawError = null;
		yamlApplyError = "";
		yamlAppliedMessage = "";
		yamlShowFullDiff = false;
	}

	function buildYamlApplyRequest(forceConflicts: boolean, yaml = yamlDraft) {
		return createYamlApplyRequest({
			resource,
			kubeconfigSourceKey,
			yaml,
			yamlEncoding,
			forceConflicts,
		});
	}

	async function startYamlApplyEdit() {
		if (yamlApplyDisabledReason || yamlLoadingDraft) return;
		yamlLoadingDraft = true;
		yamlLintDiagnostics = [];
		yamlLintNotes = [];
		yamlLintError = "";
		yamlFormatError = "";
		yamlPrepareRawError = null;
		yamlPrepareError = "";
		yamlApplyRawError = null;
		yamlApplyError = "";
		yamlAppliedMessage = "";
		yamlPreview = null;
		yamlForceConflictsForResource = false;
		yamlShowFullDiff = false;
		try {
			yamlDraft = await getResourceYaml(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
				kubeconfigSourceKey,
				"applyClean",
				yamlEncoding,
			);
			yamlEditing = true;
		} catch (error) {
			yamlPrepareRawError = error;
			yamlPrepareError = getErrorMessage(error);
		} finally {
			yamlLoadingDraft = false;
		}
	}

	async function previewYamlApply(forceConflictsOverride?: unknown) {
		if (!yamlEditing || yamlPreparing) return;
		yamlPreparing = true;
		yamlLintError = "";
		yamlFormatError = "";
		yamlPrepareRawError = null;
		yamlPrepareError = "";
		yamlApplyRawError = null;
		yamlApplyError = "";
		yamlAppliedMessage = "";
		yamlPreview = null;
		yamlShowFullDiff = false;
		const forceConflicts = resolveYamlForceConflicts(
			forceConflictsOverride,
			$settingsStore.allowYamlForceConflicts || yamlForceConflictsForResource,
		);
		try {
			yamlPreview = await prepareYamlApply(client, buildYamlApplyRequest(forceConflicts));
			yamlPreviewForceConflicts = forceConflicts;
		} catch (error) {
			yamlPrepareRawError = error;
			yamlPrepareError = getErrorMessage(error);
		} finally {
			yamlPreparing = false;
		}
	}

	function allowYamlForceConflictsForResource() {
		yamlForceConflictsForResource = true;
		void previewYamlApply(true);
	}

	async function kubernetesYamlDiagnostics(value: string): Promise<Diagnostic[]> {
		if (!yamlEditing || value.trim().length === 0) return [];
		try {
			const result = await lintKubernetesYaml(client, buildYamlApplyRequest(false, value));
			yamlLintDiagnostics = result.diagnostics;
			yamlLintNotes = result.notes;
			yamlLintError = "";
			return result.diagnostics.map((diagnostic) => {
				const range = findYamlFieldRange(value, diagnostic.fieldPath);
				return {
					from: range.from,
					to: range.to,
					severity: diagnostic.severity,
					source: diagnostic.source,
					message: diagnostic.message,
				};
			});
		} catch (error) {
			yamlLintError = getErrorMessage(error);
			return [];
		}
	}

	function formatYamlDraft() {
		if (!yamlEditing || yamlLoadingDraft || yamlPreparing || yamlApplying) return;
		clearYamlDraftFeedback();
		try {
			yamlDraft = formatYamlDocument(yamlDraft, yamlEncoding);
		} catch (error) {
			yamlFormatError = getErrorMessage(error);
		}
	}

	function clearYamlDraftFeedback() {
		yamlLintDiagnostics = [];
		yamlLintNotes = [];
		yamlLintError = "";
		yamlFormatError = "";
		yamlPrepareRawError = null;
		yamlPrepareError = "";
		yamlApplyRawError = null;
		yamlApplyError = "";
		yamlAppliedMessage = "";
		yamlPreview = null;
		yamlForceConflictsForResource = false;
		yamlShowFullDiff = false;
	}

	async function applyYamlPreview() {
		if (!yamlPreview || yamlApplying) return;
		yamlApplying = true;
		yamlApplyRawError = null;
		yamlApplyError = "";
		try {
			const result = await applyYaml(
				client,
				buildYamlApplyRequest(yamlPreviewForceConflicts),
			);
			yamlAppliedMessage = formatYamlAppliedMessage(
				result,
				yamlPreviewForceConflicts,
			);
			yamlEditing = false;
			yamlPreview = null;
			void queryClient.invalidateQueries({ queryKey: detailsQueryKey });
			void queryClient.invalidateQueries({ queryKey: yamlQueryKey });
		} catch (error) {
			yamlApplyRawError = error;
			yamlApplyError = getErrorMessage(error);
		} finally {
			yamlApplying = false;
		}
	}
</script>

<YamlTab
	{yamlQuery}
	{yamlText}
	{yamlApplyTarget}
	{yamlAppliedMessage}
	{yamlEditing}
	{yamlViewMode}
	{yamlEncoding}
	{yamlLoadingDraft}
	{yamlPreparing}
	{yamlApplying}
	bind:yamlDraft
	{yamlApplyDisabledReason}
	{yamlLintError}
	{yamlLintNotes}
	{yamlFormatError}
	{yamlPrepareRawError}
	{yamlPrepareError}
	{yamlApplyRawError}
	{yamlApplyError}
	{canAllowYamlForceConflicts}
	{yamlPreview}
	bind:yamlShowFullDiff
	{visibleYamlDiffLines}
	{hiddenYamlDiffCount}
	{yamlLintDiagnostics}
	yamlErrorLensEnabled={$settingsStore.yamlErrorLensEnabled}
	{setYamlViewMode}
	{setYamlEncoding}
	{resetYamlApply}
	{startYamlApplyEdit}
	{formatYamlDraft}
	{previewYamlApply}
	{applyYamlPreview}
	{allowYamlForceConflictsForResource}
	{kubernetesYamlDiagnostics}
	{clearYamlDraftFeedback}
/>

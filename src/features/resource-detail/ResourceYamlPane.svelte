<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import type { Diagnostic } from "@codemirror/lint";
	import {
		applyYaml,
		cancelBackendRequests,
		isAppError,
		lintKubernetesYaml,
		prepareYamlApply,
		type TauriClient,
	} from "@/lib/tauri";
	import {
		createCancelScope,
		createFiniteReadCleanup,
		createFiniteReadRequest,
	} from "@/lib/finite-read-lifecycle";
	import { diagnosticLog, diagnosticResultSummary } from "@/lib/diagnostics";
	import { withForegroundLoad } from "@/lib/foreground-loading";
	import { queryKeys } from "@/lib/queryKeys";
	import { dynamicKindKey, resourceKey } from "@/lib/resource-identity";
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
	import { readResourceYaml } from "./resourceDetailReadSpec";
	import SecretDataViewer from "./SecretDataViewer.svelte";
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
	const YAML_VIEW_MODES: YamlViewMode[] = ["kubectl", "applyClean"];
	const YAML_ENCODINGS: YamlEncoding[] = ["yaml", "kyaml"];

	const queryClient = useQueryClient();
	const finiteReadCleanup = createFiniteReadCleanup(queryClient, (cancelScope) =>
		cancelBackendRequests(client, cancelScope),
	);
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

	const selectedDynamicKindKey = $derived(dynamicKindKey(dynamicKind));
	const yamlQueryKey = $derived([
		...queryKeys.resourceYaml(
			resource,
			selectedDynamicKindKey,
			kubeconfigSourceKey,
			yamlViewMode,
			yamlEncoding,
			$settingsStore.redactSecrets,
		),
		refreshVersion,
	]);
	const yamlCancelScope = $derived(createCancelScope("resource-yaml", yamlQueryKey));
	const yamlEnabled = $derived(detailsEnabled && active);
	const showSecretDataViewer = $derived(
		resource.kind === "Secret" && !$settingsStore.redactSecrets,
	);
	const yamlApplyDisabledReason = $derived(isYamlApplyDisabled(resource));
	const yamlApplyTarget = $derived(yamlApplyTargetLabel(resource));
	const yamlForceConflictsEnabled = $derived(
		$settingsStore.allowYamlForceConflicts || yamlForceConflictsForResource,
	);
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
		diagnosticLog("detail.yaml.fetch.start", { key: resourceKey(resource) });
		const result = await withForegroundLoad(loadLabel, task);
		diagnosticLog("detail.yaml.fetch.done", {
			key: resourceKey(resource),
			ms: Math.round(performance.now() - started),
			result: diagnosticResultSummary(result),
		});
		return result;
	}


	$effect(() => {
		const currentYamlCancelScope = yamlCancelScope;
		const currentYamlQueryKey = yamlQueryKey;
		finiteReadCleanup.cancelPending(currentYamlCancelScope);
		return () =>
			finiteReadCleanup.schedule(currentYamlCancelScope, currentYamlQueryKey, {
				onError: (error) => {
					diagnosticLog("detail.yaml.cancel.error", {
						error: error instanceof Error ? error.message : String(error),
					});
				},
			});
	});

	const yamlQuery = createQuery<string>(() => ({
		queryKey: yamlQueryKey,
		queryFn: async () => {
			try {
				return await runYamlFetch("resource-yaml", async () => {
					if (dynamicKind && !showSecretDataViewer && detailsYaml) return detailsYaml;
					return await readResourceYaml(client, resource, {
						kubeconfigSourceKey,
						yamlViewMode,
						yamlEncoding,
						cancellable: createFiniteReadRequest(yamlCancelScope, "yaml"),
					});
				});
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("detail.yaml.cancel", { key: resourceKey(resource) });
				}
				throw error;
			}
		},
		enabled: yamlEnabled,
		retry: false,
		staleTime: 30_000,
		gcTime: showSecretDataViewer ? 0 : undefined,
	}));
	const yamlText = $derived(
		yamlQuery.data ?? (showSecretDataViewer ? "" : detailsYaml),
	);

	$effect(() => {
		void resource.cluster;
		void resource.kind;
		void resource.name;
		void resource.namespace;
		void kubeconfigSourceKey;
		void selectedDynamicKindKey;
		void $settingsStore.redactSecrets;
		resetYamlApply();
	});

	function setYamlViewMode(value: string) {
		const mode = YAML_VIEW_MODES.find((candidate) => candidate === value);
		if (!mode) return;
		yamlViewMode = mode;
		resetYamlApply();
	}

	function setYamlEncoding(value: string) {
		const encoding = YAML_ENCODINGS.find((candidate) => candidate === value);
		if (!encoding) return;
		yamlEncoding = encoding;
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
			yamlDraft = await readResourceYaml(client, resource, {
				kubeconfigSourceKey,
				yamlViewMode: "applyClean",
				yamlEncoding,
			});
			yamlEditing = true;
		} catch (error) {
			yamlPrepareRawError = error;
			yamlPrepareError = getErrorMessage(error);
		} finally {
			yamlLoadingDraft = false;
		}
	}

	async function previewYamlApply(forceConflictsOverride?: boolean) {
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

{#if showSecretDataViewer}
	<SecretDataViewer
		{client}
		clusterContext={resource.cluster}
		name={resource.name}
		namespace={resource.namespace}
		{kubeconfigSourceKey}
		yamlText={yamlText}
		contextKey={`${resourceKey(resource)}:${kubeconfigSourceKey ?? ""}:${$settingsStore.redactSecrets}`}
		{active}
	/>
{/if}

<YamlTab
	{yamlQuery}
	{resource}
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
	{yamlPreviewForceConflicts}
	{yamlForceConflictsEnabled}
	yamlGlobalForceConflicts={$settingsStore.allowYamlForceConflicts}
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

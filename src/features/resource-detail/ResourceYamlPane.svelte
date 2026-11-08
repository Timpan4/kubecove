<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import type { Diagnostic } from "@codemirror/lint";
	import {
		applyYaml,
		isAppError,
		lintKubernetesYaml,
		prepareYamlApply,
		type TauriClient,
	} from "@/lib/tauri";
	import {
		createFiniteReadRequest,
		finiteRead,
		finiteReadScope,
	} from "@/lib/finite-read-lifecycle";
	import { diagnosticLog, diagnosticResultSummary } from "@/lib/diagnostics";
	import { withForegroundLoad } from "@/lib/foreground-loading";
	import { queryKeys, queryMatchers } from "@/lib/queryKeys";
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
	import { clearYamlDraftNotice, registerYamlDraft } from "./yamlDraftGuard.svelte";
	import {
		buildYamlDryRunDiff,
	} from "./yamlTabDiff";
import { mapKubernetesYamlDiagnostics } from "./yamlLintDiagnostics";
	import {
		buildYamlApplyRequest as createYamlApplyRequest,
		isYamlApplyDisabled,
		resolveYamlForceConflicts,
		sameYamlApplyRequest,
		yamlAppliedMessage as formatYamlAppliedMessage,
		yamlApplyTargetLabel,
	} from "./yamlApplyModel";
	type YamlApplyRequest = ReturnType<typeof createYamlApplyRequest>;
	type YamlReview = {
		preview: YamlApplyPreview;
		request: YamlApplyRequest;
		forceConflicts: boolean;
	};
	// Each async step keeps the phase object it started from and drops its result once the phase moved on.
	type YamlApplyPhase =
		| { kind: "viewing" }
		| { kind: "loadingDraft" }
		| { kind: "editing" }
		| { kind: "preparing" }
		| { kind: "reviewed"; review: YamlReview }
		| { kind: "applying"; review: YamlReview };
	type YamlApplyFeedback =
		| { kind: "none" }
		| { kind: "formatFailed"; message: string }
		| { kind: "prepareFailed"; error: unknown }
		| { kind: "applyFailed"; error: unknown }
		| { kind: "applied"; message: string };

	let {
		client,
		resource,
		dynamicKind,
		kubeconfigSourceKey,
		detailsYaml = "",
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
	// Raw state keeps object identity, which the async steps compare against.
	let phase = $state.raw<YamlApplyPhase>({ kind: "viewing" });
	let feedback = $state.raw<YamlApplyFeedback>({ kind: "none" });
	let yamlDraft = $state("");
	// The loaded draft. The draft has unsaved changes while editing once the text differs from it.
	let yamlDraftBase = "";
	let yamlForceConflictsForResource = $state(false);
	let yamlLintDiagnostics = $state<KubernetesYamlLintDiagnostic[]>([]);
	let yamlLintNotes = $state<KubernetesYamlLintStatusNote[]>([]);
	let yamlLintError = $state("");

	const yamlEditing = $derived(
		phase.kind !== "viewing" && phase.kind !== "loadingDraft",
	);
	const yamlLoadingDraft = $derived(phase.kind === "loadingDraft");
	const yamlPreparing = $derived(phase.kind === "preparing");
	const yamlApplying = $derived(phase.kind === "applying");
	const yamlReview = $derived(
		phase.kind === "reviewed" || phase.kind === "applying" ? phase.review : null,
	);
	const yamlPreview = $derived(yamlReview?.preview ?? null);
	const yamlPreviewForceConflicts = $derived(yamlReview?.forceConflicts ?? false);
	const yamlFormatError = $derived(feedback.kind === "formatFailed" ? feedback.message : "");
	const yamlPrepareRawError = $derived(feedback.kind === "prepareFailed" ? feedback.error : null);
	const yamlPrepareError = $derived(
		feedback.kind === "prepareFailed" ? getErrorMessage(feedback.error) : "",
	);
	const yamlApplyRawError = $derived(feedback.kind === "applyFailed" ? feedback.error : null);
	const yamlApplyError = $derived(
		feedback.kind === "applyFailed" ? getErrorMessage(feedback.error) : "",
	);
	const yamlAppliedMessage = $derived(feedback.kind === "applied" ? feedback.message : "");

	const selectedDynamicKindKey = $derived(dynamicKindKey(dynamicKind));
	const yamlQueryKey = $derived(
		queryKeys.resourceYaml(
			resource,
			kubeconfigSourceKey,
			{
				dynamicKindKey: selectedDynamicKindKey,
				yamlViewMode,
				yamlEncoding,
				redactSecrets: $settingsStore.redactSecrets,
			},
			refreshVersion,
		),
	);
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


	const yamlQuery = createQuery<string>(() => ({
		...finiteRead({
			queryKey: yamlQueryKey,
			label: "detail.yaml",
			read: (cancellable) =>
				runYamlFetch("resource-yaml", async () => {
					if (dynamicKind && !showSecretDataViewer && detailsYaml) return detailsYaml;
					return await readResourceYaml(client, resource, {
						kubeconfigSourceKey,
						yamlViewMode,
						yamlEncoding,
						cancellable,
					});
				}),
		}),
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
		void resource.apiVersion;
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

	function clearYamlLint() {
		yamlLintDiagnostics = [];
		yamlLintNotes = [];
		yamlLintError = "";
	}

	$effect(() => registerYamlDraft(() => yamlEditing && yamlDraft !== yamlDraftBase));

	function resetYamlApply() {
		phase = { kind: "viewing" };
		feedback = { kind: "none" };
		yamlDraft = "";
		yamlDraftBase = "";
		clearYamlDraftNotice();
		yamlForceConflictsForResource = false;
		clearYamlLint();
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
		if (yamlApplyDisabledReason || phase.kind !== "viewing") return;
		const loading: YamlApplyPhase = { kind: "loadingDraft" };
		phase = loading;
		feedback = { kind: "none" };
		clearYamlLint();
		yamlForceConflictsForResource = false;
		yamlShowFullDiff = false;
		try {
			const draft = await readResourceYaml(client, resource, {
				kubeconfigSourceKey,
				yamlViewMode: "applyClean",
				yamlEncoding,
				cancellable: createFiniteReadRequest(finiteReadScope(yamlQueryKey), "yaml-draft"),
			});
			if (phase !== loading) return;
			yamlDraftBase = draft;
			yamlDraft = draft;
			phase = { kind: "editing" };
		} catch (error) {
			if (phase !== loading) return;
			phase = { kind: "viewing" };
			feedback = { kind: "prepareFailed", error };
		}
	}

	async function previewYamlApply(forceConflictsOverride?: boolean) {
		if (phase.kind !== "editing" && phase.kind !== "reviewed") return;
		const preparing: YamlApplyPhase = { kind: "preparing" };
		phase = preparing;
		feedback = { kind: "none" };
		yamlLintError = "";
		yamlShowFullDiff = false;
		const forceConflicts = resolveYamlForceConflicts(
			forceConflictsOverride,
			$settingsStore.allowYamlForceConflicts || yamlForceConflictsForResource,
		);
		const request = buildYamlApplyRequest(forceConflicts);
		try {
			const preview = await prepareYamlApply(client, request);
			if (phase !== preparing) return;
			phase = { kind: "reviewed", review: { preview, request, forceConflicts } };
		} catch (error) {
			if (phase !== preparing) return;
			phase = { kind: "editing" };
			feedback = { kind: "prepareFailed", error };
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
			return mapKubernetesYamlDiagnostics(value, result.diagnostics);
		} catch (error) {
			yamlLintError = getErrorMessage(error);
			return [];
		}
	}

	function formatYamlDraft() {
		if (phase.kind !== "editing" && phase.kind !== "reviewed") return;
		clearYamlDraftFeedback();
		try {
			yamlDraft = formatYamlDocument(yamlDraft, yamlEncoding);
		} catch (error) {
			feedback = { kind: "formatFailed", message: getErrorMessage(error) };
		}
	}

	/** Any draft change invalidates the dry run. The editor is inert while an apply runs. */
	function clearYamlDraftFeedback() {
		if (!yamlEditing || phase.kind === "applying") return;
		phase = { kind: "editing" };
		feedback = { kind: "none" };
		clearYamlLint();
		yamlForceConflictsForResource = false;
		yamlShowFullDiff = false;
	}

	async function applyYamlPreview() {
		if (phase.kind !== "reviewed") return;
		const { review } = phase;
		if (!sameYamlApplyRequest(review.request, buildYamlApplyRequest(review.forceConflicts))) {
			return;
		}
		const applying: YamlApplyPhase = { kind: "applying", review };
		phase = applying;
		feedback = { kind: "none" };
		// Captured before the await: the cluster changed even if the pane moved to another resource.
		const appliedQueries = queryMatchers.yamlApply(resource, kubeconfigSourceKey, {
			dynamicKindKey: selectedDynamicKindKey,
			yamlViewMode,
			yamlEncoding,
		});
		try {
			const result = await applyYaml(client, review.request);
			void queryClient.invalidateQueries({ predicate: appliedQueries });
			if (phase !== applying) return;
			phase = { kind: "viewing" };
			feedback = {
				kind: "applied",
				message: formatYamlAppliedMessage(result, review.forceConflicts),
			};
		} catch (error) {
			if (phase !== applying) return;
			const draftUnchanged = sameYamlApplyRequest(
				review.request,
				buildYamlApplyRequest(review.forceConflicts),
			);
			phase = draftUnchanged ? { kind: "reviewed", review } : { kind: "editing" };
			feedback = { kind: "applyFailed", error };
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

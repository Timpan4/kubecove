<script lang="ts">
	import { getArgoOperationTracker, argoTrackingKey } from "./argo-operation-tracker";
	import { argoOperationProgress } from "./argo-operation-progress";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import ArgoApplicationDelivery from "./ArgoApplicationDelivery.svelte";
	import { argoBooleanAt as booleanAt, argoStatusValue as statusValue, argoStringAt as stringAt } from "./argo-application-display";
	import ArgoApplicationConnection from "./ArgoApplicationConnection.svelte";
	import ArgoOperationProgress from "./ArgoOperationProgress.svelte";
	import ArgoReconciliationPane from "./ArgoReconciliationPane.svelte";
	import { useQueryClient } from "@tanstack/svelte-query";
	import {
		ChevronDown,
		CircleDot,
		RefreshCw,
		RotateCcw,
		TriangleAlert,
	} from "lucide-svelte";
	import HealthAssessmentBadge from "@/components/HealthAssessmentBadge.svelte";
	import OperationScopeCard from "@/components/OperationScopeCard.svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Checkbox,
		Dialog,
		DialogContent,
		DialogDescription,
		DialogHeader,
		DialogTitle,
		Field,
		FieldLabel,
		Input,
		Popover,
		PopoverContent,
		PopoverTrigger,
		Spinner,
		buttonClass,
	} from "@/components/ui/svelte";
	import { applicationSpecFromYaml } from "./argo-application-yaml";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import { argoApplicationRef } from "@/lib/argo-application-inspection";
	import { argoApplicationInspection } from "@/lib/argo-application-inspection.svelte";
	import { createTauriClient } from "@/lib/tauri";
	import type {
		ArgoApplicationRef,
		ArgoApplicationSummary,
		ArgoOperationAction,
		ArgoOperationRequest,
		ResourceDetailsFull,
		ResourceSummary,
	} from "@/lib/types";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		argoOperationAvailability,
		argoOperationBlocker,
		argoOperationTarget,
	} from "./argo-operation-presentation";
	import {
		applyArgoSyncDefaults,
		argoHistoryKey,
		argoResourceCounts,
		argoSyncNeedsConfirmation,
		defaultArgoSyncSettings,
		preserveArgoHistorySelection,
		withArgoSyncSettings,
		type ArgoSyncSettings,
	} from "./argo-workspace-model";

	let {
		resourceSummary,
		applicationSummary = null,
		applicationDetails = null,
		workspaceReadContext,
		active,
	}: {
		resourceSummary: ResourceSummary;
		applicationSummary?: ArgoApplicationSummary | null;
		applicationDetails?: ResourceDetailsFull | null;
		workspaceReadContext: WorkspaceReadContext;
		active: boolean;
	} = $props();

	const client = createTauriClient();
	const queryClient = useQueryClient();
	const tracker = getArgoOperationTracker(queryClient);

	const clusterContext = $derived(workspaceReadContext.clusterContext);
	const workspaceId = $derived(workspaceReadContext.workspaceId);
	const kubeconfigEnvVar = $derived(workspaceReadContext.kubeconfigSourceKey);
	const matchingSummary = $derived(
		applicationSummary &&
			applicationSummary.cluster === clusterContext &&
			applicationSummary.name === resourceSummary.name &&
			(applicationSummary.namespace ?? "") === (resourceSummary.namespace ?? "")
			? applicationSummary
			: null,
	);
	const applicationRequest = $derived(
		argoApplicationRef(workspaceReadContext, {
			name: resourceSummary.name,
			namespace: resourceSummary.namespace,
			project: matchingSummary?.project,
			resourceVersion: matchingSummary?.resourceVersion,
			uid: matchingSummary?.uid,
			apiVersion: resourceSummary.apiVersion,
		}),
	);
	const trackingKey = $derived(argoTrackingKey(applicationRequest, kubeconfigEnvVar));
	const trackedOperation = $derived($tracker.get(trackingKey));
	const operationPhase = $derived(trackedOperation?.phase === "pending" ? "refreshing" : trackedOperation?.phase ?? "idle");
	const operationMessage = $derived(trackedOperation?.message ?? null);
	const operationError = $derived(trackedOperation?.error ?? null);
	const observationPending = $derived(trackedOperation?.phase === "unknown");
	const lastOperationRequest = $derived(trackedOperation?.request ?? null);

	let selectedHistoryKey = $state<string | null>(null);
	let rollbackHistoryId = $state<number | null>(null);
	let refreshMenuOpen = $state(false);
	let syncMenuOpen = $state(false);
	let advancedSync = $state<ArgoSyncSettings>({ ...defaultArgoSyncSettings });
	let appliedSyncDefaults = $state<ArgoSyncSettings>({ ...defaultArgoSyncSettings });
	let appliedSyncDefaultsKey = $state("");
	let confirmationName = $state("");
	// The advanced-sync confirmation is open exactly while a sync waits for it.
	let pendingSync = $state<ArgoSyncSettings | null>(null);
	const confirmationOpen = $derived(pendingSync !== null);
	let appliedScopeKey = $state("");

	const inspection = argoApplicationInspection(client, () => ({
		readContext: workspaceReadContext,
		application: applicationRequest,
		enabled: active,
	}));
	const snapshot = $derived(inspection.current.data);
	const connectionPolicy = $derived(inspection.current.choice);
	const transport = $derived(connectionPolicy.transport);
	const connectionId = $derived(connectionPolicy.connectionId ?? "");
	const connectionReady = $derived(inspection.current.ready);
	const selectedStatus = $derived(inspection.current.selectedStatus);
	const application = $derived<ArgoApplicationRef>({
		...applicationRequest,
		...snapshot?.application,
		context: clusterContext,
		workspaceId,
	});
	const managedResources = $derived(snapshot?.resources ?? []);
	const scopeKey = $derived(
		[
			clusterContext,
			workspaceId,
			kubeconfigEnvVar ?? "",
			resourceSummary.namespace ?? "",
			resourceSummary.name,
			applicationRequest.uid ?? "",
		].join(":"),
	);
	const selectedHistory = $derived(
		(snapshot?.history ?? []).find(
			(entry) => argoHistoryKey(application, entry) === selectedHistoryKey,
		) ?? snapshot?.history[0] ?? null,
	);
	const counts = $derived(argoResourceCounts(managedResources));
	const healthStatus = $derived(
		statusValue(snapshot?.status, "health", "status") ??
			matchingSummary?.healthStatus ??
			resourceSummary.status ??
			"Unknown",
	);
	const syncStatus = $derived(
		statusValue(snapshot?.status, "sync", "status") ??
			matchingSummary?.syncStatus ??
			"Unknown",
	);
	const healthAssessment = $derived(
		matchingSummary?.healthAssessment ?? resourceSummary.healthAssessment,
	);
	const resolvedRevision = $derived(
		statusValue(snapshot?.status, "sync", "revision") ??
			matchingSummary?.sources?.[0]?.resolvedRevision ??
			selectedHistory?.revision ??
			matchingSummary?.sourceRevision ??
			"Revision unavailable",
	);
	const operationStateMessage = $derived(
		statusValue(snapshot?.operationState, "phase") ??
			statusValue(snapshot?.operationState, "message"),
	);
	const conditionRows = $derived(
		(snapshot?.conditions ?? []).flatMap((condition) => {
			const type = statusValue(condition, "type");
			const message = statusValue(condition, "message");
			return type || message ? [{ type: type ?? "Application condition", message: message ?? "No condition detail reported." }] : [];
		}),
	);
	const applicationSpec = $derived(applicationSpecFromYaml(applicationDetails?.yaml));
	const primarySource = $derived(matchingSummary?.sources?.[0] ?? null);
	const configuredRevision = $derived(
		primarySource?.targetRevision ??
			matchingSummary?.sourceRevision ??
			stringAt(applicationSpec, "source", "targetRevision") ??
			stringAt(applicationSpec, "sources", "0", "targetRevision") ??
			"Revision unavailable",
	);
	const applicationSyncDefaults = $derived<ArgoSyncSettings>({
		revision: "",
		prune: booleanAt(applicationSpec, "syncPolicy", "automated", "prune") ?? false,
		dryRun: false,
		force: false,
	});
	const loading = $derived(inspection.current.loading);
	const loadingIndicator = loadingDelay(() => loading);
	const dataError = $derived(inspection.current.error);
	const busy = $derived(
		Boolean(trackedOperation?.busy || (snapshot && (argoOperationProgress(snapshot).active || snapshot.refreshRequested))),
	);
	const operationTarget = $derived(
		argoOperationTarget(resourceSummary, clusterContext, transport),
	);
	const operationAvailability = $derived(argoOperationAvailability({
		sourceReady: workspaceReadContext.sourceReady,
		connectionReady,
		transport,
		connected: selectedStatus?.connected === true,
		unavailableReason: selectedStatus?.unavailableReason ?? undefined,
	}));
	const canOperate = $derived(active && operationAvailability.available);
	const operationBlocker = $derived(operationError ? argoOperationBlocker(operationError) : null);

	$effect(() => {
		const nextScopeKey = scopeKey;
		if (appliedScopeKey && appliedScopeKey !== nextScopeKey) {
			selectedHistoryKey = null;
			rollbackHistoryId = null;
			closeSyncConfirmation();
			appliedSyncDefaultsKey = "";
		}
		appliedScopeKey = nextScopeKey;
	});
	$effect(() => {
		const next = preserveArgoHistorySelection(
			application,
			snapshot?.history ?? [],
			selectedHistoryKey,
		);
		if (next !== selectedHistoryKey) selectedHistoryKey = next;
	});
	$effect(() => {
		const next = { ...applicationSyncDefaults };
		const key = JSON.stringify(next);
		if (appliedSyncDefaultsKey !== key) {
			advancedSync = appliedSyncDefaultsKey
				? applyArgoSyncDefaults(advancedSync, appliedSyncDefaults, next)
				: next;
			appliedSyncDefaults = next;
			appliedSyncDefaultsKey = key;
		}
	});

	function operation(action: ArgoOperationAction): ArgoOperationRequest {
		return {
			connectionId: transport === "connected" ? connectionId : null,
			transport,
			application,
			action,
			revision: null,
			resources: [],
			prune: null,
			dryRun: null,
			force: null,
			historyId: null,
			resourceAction: null,
			resourceActionParameters: null,
			resourceVersion: application.resourceVersion ?? null,
			clusterContext,
			kubeconfigEnvVar,
		};
	}

	let review = $state<import("@/lib/types").ArgoOperationPreflight | null>(null);
	let resolveReview: ((confirmed: boolean) => void) | undefined;
	function closeOperationReview(confirmed = false) {
		resolveReview?.(confirmed);
		resolveReview = undefined;
		review = null;
	}
	$effect(() => {
		void scopeKey;
		return () => closeOperationReview();
	});
	async function executeOperation(requested: ArgoOperationRequest) {
		if (!busy) await tracker.run(requested, (next) => new Promise<boolean>((resolve) => {
			review = next;
			resolveReview = resolve;
		}));
	}

	async function retryAcceptedRefresh() {
		await tracker.observe(trackingKey);
	}

	function refresh(hard = false) {
		refreshMenuOpen = false;
		void executeOperation(operation(hard ? "hardRefresh" : "refresh"));
	}

	function syncWithDefaults() {
		void executeOperation(
			withArgoSyncSettings(operation("sync"), applicationSyncDefaults),
		);
	}

	function selectHistory(value: string) {
		selectedHistoryKey = value;
		rollbackHistoryId = snapshot?.history.find(entry => argoHistoryKey(application, entry) === value)?.id ?? null;
	}

	function rollbackPrevious() {
		const historyId = rollbackHistoryId ?? snapshot?.history[1]?.id;
		if (historyId != null) void executeOperation({ ...operation("rollback"), historyId });
	}

	function requestAdvancedSync() {
		syncMenuOpen = false;
		const requested = { ...advancedSync };
		if (argoSyncNeedsConfirmation(requested, applicationSyncDefaults)) {
			confirmationName = "";
			pendingSync = requested;
			return;
		}
		void executeOperation(withArgoSyncSettings(operation("sync"), requested));
	}

	function closeSyncConfirmation() {
		pendingSync = null;
		confirmationName = "";
	}

	function confirmAdvancedSync() {
		if (!pendingSync || confirmationName !== resourceSummary.name) return;
		const requested = withArgoSyncSettings(operation("sync"), pendingSync);
		closeSyncConfirmation();
		void executeOperation(requested);
	}

	function retryOperation() {
		if (lastOperationRequest && !busy) void executeOperation(lastOperationRequest);
	}


</script>

<div class="@container flex min-h-0 min-w-0 flex-col gap-3 [&_button]:motion-reduce:transition-none" data-active={active}>
	<section class="overflow-hidden rounded-lg border bg-surface-1 shadow-sm">
		<div class="resource-tone-argo-surface p-3">
			<div class="flex flex-wrap items-start justify-between gap-3">
				<div class="min-w-0">
					<div class="flex min-w-0 flex-wrap items-center gap-1.5">
						<h2 class="max-w-full truncate font-heading text-sm font-semibold">{resourceSummary.name}</h2>
						<Badge variant="ghost">
							<CircleDot class="size-2.5 text-primary" />
							{snapshot?.transport === "connected"
								? "Connected Argo CD API"
								: "Kubernetes Application status"}
							{#if snapshot?.provenance} · {snapshot.provenance}{/if}
						</Badge>
					</div>
					<div class="mt-2">
						<HealthAssessmentBadge assessment={healthAssessment} {loading} details />
					</div>
					<div class="mt-2 flex flex-wrap items-center gap-2">
						<span class="text-xs text-muted-foreground">Raw Argo:</span>
						<Badge variant="outline">Health {healthStatus}</Badge>
						<Badge variant="outline">Sync {syncStatus}</Badge>
						{#if operationStateMessage}<Badge variant="secondary">{operationStateMessage}</Badge>{/if}
					</div>
					<div class="mt-1 max-w-xl text-xs leading-relaxed text-muted-foreground">
						{loadingIndicator.shown ? "Loading Application and managed resources…" : loadingIndicator.active ? "" : `Revision ${resolvedRevision}`}
					</div>
					<OperationScopeCard
						class="mt-3"
						context={operationTarget.context}
						namespace={operationTarget.namespace}
						kind={operationTarget.kind}
						resource={operationTarget.resource}
						operationScope={operationTarget.operationScope}
						transport={operationTarget.transport}
						description={`${operationAvailability.available ? operationAvailability.reason : `Unavailable, ${operationAvailability.blocker}: ${operationAvailability.reason}`} These operations target the Argo CD Application, not a selected managed Kubernetes resource.`}
					/>
				</div>
				<div class="flex max-w-full shrink-0 flex-wrap gap-1.5">
					<Button type="button" size="sm" variant="outline" disabled={busy || !snapshot?.history[1]} onclick={rollbackPrevious}>Rollback</Button>
					<div class="inline-flex shrink-0">
						<Button type="button" size="sm" variant="outline" class="rounded-r-none" disabled={!canOperate || busy} onclick={() => refresh(false)}>
							{#if operationPhase === "refreshing" && lastOperationRequest?.action !== "sync"}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}
							Refresh
						</Button>
						<Popover bind:open={refreshMenuOpen}>
							<PopoverTrigger type="button" class={buttonClass({ variant: "outline", size: "icon-sm", className: "rounded-l-none border-l-0" })} aria-label="Refresh options" disabled={!canOperate || busy}><ChevronDown /></PopoverTrigger>
							<PopoverContent align="end" class="w-64 space-y-2">
								<div class="text-sm font-semibold">Refresh Application</div>
								<div class="text-xs leading-relaxed text-muted-foreground">Normal refresh re-reads Application state. Hard refresh also invalidates Argo CD manifest and repository caches.</div>
								<Button type="button" variant="outline" size="sm" class="w-full" disabled={!canOperate || busy} onclick={() => refresh(true)}><RotateCcw data-icon="inline-start" /> Hard refresh</Button>
							</PopoverContent>
						</Popover>
					</div>
					<div class="inline-flex shrink-0">
						<Button type="button" size="sm" class="rounded-r-none" disabled={!canOperate || busy} onclick={syncWithDefaults}>
							{#if busy && lastOperationRequest?.action === "sync"}<Spinner />{:else}<RotateCcw data-icon="inline-start" />{/if}
							Sync
						</Button>
						<Popover bind:open={syncMenuOpen}>
							<PopoverTrigger type="button" class={buttonClass({ size: "icon-sm", className: "rounded-l-none border-l border-primary-foreground/20" })} aria-label="Sync options" disabled={!canOperate || busy}><ChevronDown /></PopoverTrigger>
							<PopoverContent align="end" class="w-80 max-w-[calc(100vw-2rem)] space-y-3">
								<div><div class="text-sm font-semibold">Sync with options</div><div class="mt-0.5 text-xs leading-relaxed text-muted-foreground">Main Sync uses Application defaults. Choices beyond those defaults require confirmation.</div></div>
								<Field><FieldLabel>Revision override</FieldLabel><Input value={advancedSync.revision} placeholder={configuredRevision} oninput={(event: Event) => advancedSync = { ...advancedSync, revision: (event.currentTarget as HTMLInputElement).value }} /></Field>
								<div class="grid grid-cols-2 gap-2">
									<label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.prune} onCheckedChange={(checked) => advancedSync = { ...advancedSync, prune: checked === true }} />Prune</label>
									<label class="flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.dryRun} onCheckedChange={(checked) => advancedSync = { ...advancedSync, dryRun: checked === true }} />Dry run</label>
									<label class="col-span-2 flex items-center gap-2 rounded-md border p-2 text-xs"><Checkbox checked={advancedSync.force} onCheckedChange={(checked) => advancedSync = { ...advancedSync, force: checked === true }} />Force / replace</label>
								</div>
								<Button type="button" class="w-full" disabled={!canOperate || busy} onclick={requestAdvancedSync}>Sync with these options</Button>
							</PopoverContent>
						</Popover>
					</div>
				</div>
			</div>
			<div class="mt-3 grid grid-cols-3 gap-px overflow-hidden rounded-md border bg-border">
				{@render SignalCard(counts.needsSync, "Out of sync", "Needs reconciliation")}
				{@render SignalCard(counts.degraded + counts.progressing, "Needs attention", "Health is not settled")}
				{@render SignalCard(counts.prune, "Prune", "No longer in target")}
			</div>
		</div>
	</section>

	{#if snapshot?.connectedFallback}
		<Alert>
			<AlertTitle>Connected inspection unavailable</AlertTitle>
			<AlertDescription>
				Kubernetes fallback snapshot shown. Connected
				{snapshot.connectedFallback.failure.kind}:
				{snapshot.connectedFallback.failure.message}. Kubernetes watch keeps this snapshot fresh.
			</AlertDescription>
		</Alert>
	{/if}

	{#if review?.reviewedRequest}
		<section aria-label="Review Argo CD operation" class="rounded-lg border bg-surface-1 p-3 text-xs space-y-2">
			<h3>Review Argo CD operation</h3>
			<p>{review.action} {review.transport} {review.reviewedRequest.application.context} Application {review.reviewedRequest.application.namespace} {review.reviewedRequest.application.name}</p>
			<p>Revision: {review.reviewedRequest.revision ?? "Application default"}. History: {review.reviewedRequest.historyId ?? "none"}. Prune: {String(review.reviewedRequest.prune)}. Dry run: {String(review.reviewedRequest.dryRun)}. Force: {String(review.reviewedRequest.force)}.</p>
			<Button size="sm" onclick={() => closeOperationReview(true)}>Confirm Argo CD operation</Button>
			<Button size="sm" variant="outline" onclick={() => closeOperationReview()}>Cancel review</Button>
		</section>
	{/if}

	<ArgoApplicationConnection {inspection} {workspaceReadContext} />

	{#if snapshot}<ArgoOperationProgress inspector={snapshot} />{/if}
	{#if operationMessage}
		<div class="flex items-center gap-2 rounded-lg border bg-surface-1 p-3 text-xs" role="status" aria-live="polite" aria-atomic="true">
			{#if busy}<Spinner class="size-3.5" />{/if}<span>{operationMessage}</span>
		</div>
	{/if}
	{#if operationError}
		<Alert variant="destructive">
			<AlertTitle>
				{observationPending
					? "Operation status unavailable"
					: operationBlocker === "permission"
						? "Permission blocker"
						: operationBlocker === "provider connection"
							? "Provider connection blocker"
							: "Operation support blocker"}
			</AlertTitle>
			<AlertDescription class="flex flex-wrap items-center justify-between gap-2"><span>{operationError}</span>{#if observationPending}<Button type="button" size="sm" variant="outline" onclick={() => void retryAcceptedRefresh()}>Retry state refresh</Button>{:else if lastOperationRequest}<Button type="button" size="sm" variant="outline" disabled={busy} onclick={retryOperation}>Retry operation</Button>{/if}</AlertDescription>
		</Alert>
	{/if}
	{#if dataError}
		<FriendlyError
			error={dataError}
			context={{
				operation: "detailsLoad",
				fallbackTitle: transport === "connected" ? "Connected Argo CD snapshot unavailable" : "Kubernetes Application snapshot unavailable",
			}}
		/>
		<Button type="button" size="sm" variant="outline" onclick={() => void inspection.refresh()}>Retry snapshot</Button>
	{/if}

	{#each conditionRows as condition}
		<div class="flex items-start gap-2 rounded-lg border border-amber-500/25 bg-amber-500/5 p-3 text-xs">
			<TriangleAlert class="mt-0.5 size-3.5 shrink-0 text-amber-400" />
			<div class="min-w-0"><div class="font-medium">{condition.type}</div><div class="mt-0.5 break-words leading-relaxed text-muted-foreground">{condition.message}</div></div>
		</div>
	{/each}

	<ArgoReconciliationPane
		{client} {applicationRequest} {scopeKey} {clusterContext} {workspaceId} {kubeconfigEnvVar}
		{managedResources} {snapshot} request={inspection.current.request} sourceReady={workspaceReadContext.sourceReady}
		{connectionReady} {transport} {connectionId} {active} {loading} {dataError}
	>
		{#snippet delivery()}
			<ArgoApplicationDelivery {matchingSummary} {applicationSpec} {configuredRevision}
				history={snapshot?.history ?? []} {application} {selectedHistoryKey} {selectedHistory} {selectHistory} />
		{/snippet}
	</ArgoReconciliationPane>

</div>

<Dialog open={confirmationOpen} onOpenChange={(open: boolean) => { if (!open) closeSyncConfirmation(); }}>
	<DialogContent>
		<DialogHeader><DialogTitle>Confirm advanced sync</DialogTitle><DialogDescription>These choices go beyond Application defaults. Type Application name to continue.</DialogDescription></DialogHeader>
		{#if pendingSync}
			<div class="grid grid-cols-[5rem_minmax(0,1fr)] gap-x-3 gap-y-1 rounded-md border bg-muted/20 p-3 text-xs">
				<span class="text-muted-foreground">Revision</span><span class="break-all">{pendingSync.revision || configuredRevision}</span>
				<span class="text-muted-foreground">Prune</span><span>{pendingSync.prune ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Dry run</span><span>{pendingSync.dryRun ? "Enabled" : "Disabled"}</span>
				<span class="text-muted-foreground">Force</span><span>{pendingSync.force ? "Enabled" : "Disabled"}</span>
			</div>
		{/if}
		<Field><FieldLabel>Type {resourceSummary.name} to confirm</FieldLabel><Input bind:value={confirmationName} /></Field>
		<div class="flex justify-end gap-2"><Button type="button" variant="outline" onclick={closeSyncConfirmation}>Cancel</Button><Button type="button" disabled={confirmationName !== resourceSummary.name || !pendingSync} onclick={confirmAdvancedSync}>Confirm sync</Button></div>
	</DialogContent>
</Dialog>

{#snippet SignalCard(value: number, label: string, description: string)}
	<div class="bg-surface-1 p-2.5"><div class="flex items-baseline gap-1.5"><div class="text-base font-semibold tabular-nums">{value}</div><div class="text-xs font-medium">{label}</div></div><div class="mt-0.5 text-[0.65rem] text-muted-foreground">{description}</div></div>
{/snippet}

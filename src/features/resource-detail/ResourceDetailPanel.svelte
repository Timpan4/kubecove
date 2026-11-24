<script module lang="ts">
	const loadYamlPane = () => import("./ResourceYamlPane.svelte");
	const loadArgoPane = () => import("@/features/gitops/ArgoApplicationDetails.svelte");
	let execTabImport: Promise<typeof import("./ExecTab.svelte")> | undefined;

	function loadExecTab() {
		return (execTabImport ??= import("./ExecTab.svelte"));
	}

	function retryExecTabLoad() {
		execTabImport = undefined;
		return loadExecTab();
	}
</script>

<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { untrack } from "svelte";
	import { SvelteSet } from "svelte/reactivity";
	import { Button, Spinner, Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/svelte";
	import { loadingDelay } from "@/lib/loading-delay.svelte";
	import {
		formatExactTimeOnly,
		formatExactTimestamp,
	} from "@/components/timestamp-format";
	import { messageFromError } from "@/lib/error-redaction";
	import { diagnosticLog } from "@/lib/diagnostics";
	import type { TauriClient } from "@/lib/tauri";
	import DeferredSurface from "@/components/DeferredSurface.svelte";
	import { resourceDetailWatches } from "./resourceDetailWatches.svelte";
	import type {
		ArgoApplicationSummary,
		ResourceDetailsFull,
		ResourceEventSummary,
		ResourceSummary,
		YamlEncoding,
		YamlViewMode,
	} from "@/lib/types";
	import type { PathStateResourceDetailState } from "@/lib/path-state";
	import { argoApplicationReads } from "@/lib/argo-application-inspection";
	import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		getSettingsSnapshot,
		settingsStore,
	} from "@/lib/settings-store";
	import {
		buildIncidentSignals,
		getConditionRows,
		getContainerStatusRows,
	} from "./helpers";
	import {
		buildResourceDetailReadSpec,
		resourceDetailsQuery,
		resourceEventsQuery,
	} from "./resourceDetailReadSpec";
	import DetailsTab from "./DetailsTab.svelte";
	import DeploymentRevisionsTab from "./DeploymentRevisionsTab.svelte";
	import EventsTab from "./EventsTab.svelte";
	import PortForwardTab from "./PortForwardTab.svelte";
	import ResourceActionsTab from "./ResourceActionsTab.svelte";
	import ResourceLogsPane from "./ResourceLogsPane.svelte";
	import { sortIncidentEvents } from "./incident-events";
	import {
		buildIncidentTimeline,
	} from "./incident-timeline";
	import type { ParsedLogLine } from "./log-helpers";
	import {
		buildCuratedMetadata,
		formatObjectRows,
		visibleMetadataBadges,
	} from "./metadata-details";

	type DetailTab =
		| "details"
		| "yaml"
		| "events"
		| "logs"
		| "exec"
		| "portForward"
		| "revisions"
		| "operations"
		| "argo";
	const DETAIL_TABS: DetailTab[] = [
		"details", "yaml", "events", "logs", "exec", "portForward", "revisions", "operations", "argo",
	];

	let {
		client,
		resource,
		workspaceReadContext,
		argoApplicationSummary = null,
		onOpenHelmRelease,
		initialPathState = null,
		onPathStateChange = () => {},
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		workspaceReadContext: WorkspaceReadContext;
		argoApplicationSummary?: ArgoApplicationSummary | null;
		onOpenHelmRelease?: (releaseName: string, namespace: string | null, cluster: string) => void;
		initialPathState?: PathStateResourceDetailState | null;
		onPathStateChange?: (state: PathStateResourceDetailState) => void;
	} = $props();

	const queryClient = useQueryClient();
	const kubeconfigSourceKey = $derived(workspaceReadContext.kubeconfigSourceKey);
	const timestampTimezone = $derived($settingsStore.timestampTimezone);
	function initialDetailSnapshot(): PathStateResourceDetailState | null {
		return initialPathState;
	}
	const initialDetailState = initialDetailSnapshot();
	let yamlViewMode = $state<YamlViewMode>(
		initialDetailState?.yamlViewMode ?? getSettingsSnapshot().yamlViewModeDefault,
	);
	let yamlEncoding = $state<YamlEncoding>(
		initialDetailState?.yamlEncoding ?? getSettingsSnapshot().yamlEncodingDefault,
	);
	let activeTab = $state<DetailTab>(initialDetailState?.activeTab ?? "details");
	let appliedInitialActiveTab = $state<DetailTab | null>(initialDetailState?.activeTab ?? null);
	let metadataLabelsExpanded = $state(initialDetailState?.metadataLabelsExpanded ?? false);
	let metadataAnnotationsExpanded = $state(
		initialDetailState?.metadataAnnotationsExpanded ?? false,
	);
	let selectedContainer = $state(initialDetailState?.selectedContainer ?? "");
	let logFilter = $state(initialDetailState?.logFilter ?? "");
	let logWrapLines = $state(initialDetailState?.logWrapLines ?? true);
	let logLatestFirst = $state(initialDetailState?.logLatestFirst ?? false);
	let parsedLogLines = $state.raw<ParsedLogLine[]>([]);
	const visitedTabs = new SvelteSet<DetailTab>();
	$effect(() => {
		visitedTabs.add(activeTab);
	});
	const showEventsWarnings = $derived(
		activeTab === "details" || activeTab === "events",
	);
	let yamlShowFullDiff = $state(initialDetailState?.yamlShowFullDiff ?? false);
	let resourceRefreshVersion = $state(0);
	let ExecTabComponent = $state<typeof import("./ExecTab.svelte").default | null>(null);
	let execTabLoadError = $state<unknown>(null);
	const execModuleLoading = loadingDelay(() => activeTab === "exec" && !ExecTabComponent && !execTabLoadError);

	const readSpec = $derived(
		buildResourceDetailReadSpec(resource, kubeconfigSourceKey, yamlViewMode, yamlEncoding),
	);
	const resourceKey = $derived(readSpec.identity.key);
	const dynamicKind = $derived(readSpec.dynamicKind);
	const detailReadPermission = $derived(readSpec.detailReadPermission);
	const detailsEnabled = $derived(readSpec.detailsEnabled);
	const eventsEnabled = $derived(readSpec.eventsEnabled);
	const isPod = $derived(resource.kind === "Pod" && Boolean(resource.namespace));
	const canShowLogs = $derived(
		Boolean(resource.namespace) &&
			(resource.kind === "Pod" || resource.kind === "Deployment" || resource.kind === "Service"),
	);
	const canShowExec = $derived(isPod);
	const canShowRevisions = $derived(resource.kind === "Deployment" && Boolean(resource.namespace));
	const canShowPortForward = $derived(
		(resource.kind === "Pod" || resource.kind === "Service") && Boolean(resource.namespace),
	);
	const isArgoApplication = $derived(
		resource.kind === "Application" && resource.group === "argoproj.io",
	);
	const argoApplicationReadsMatch = $derived(
		isArgoApplication ? argoApplicationReads(workspaceReadContext, resource) : null,
	);

	$effect(() => {
		if (activeTab !== "exec" || ExecTabComponent || execTabLoadError) return;
		void loadExecTab()
			.then((module) => {
				ExecTabComponent = module.default;
			})
			.catch((cause: unknown) => {
				execTabLoadError = cause;
			});
	});

	function retryExecTab() {
		execTabLoadError = null;
		void retryExecTabLoad()
			.then((module) => {
				ExecTabComponent = module.default;
			})
			.catch((cause: unknown) => {
				execTabLoadError = cause;
			});
	}
	const watches = resourceDetailWatches(
		() => client,
		queryClient,
		() => detailsEnabled ? {
			resource, watchKey: readSpec.resourceWatchKey, detailsQueryKey: readSpec.detailsQueryKey,
			argoReads: argoApplicationReadsMatch, source: kubeconfigSourceKey,
		} : null,
		() => eventsEnabled ? {
			key: resourceKey, source: kubeconfigSourceKey, read: untrack(() => readSpec),
		} : null,
		() => { if (activeTab === "yaml") resourceRefreshVersion += 1; },
	);

	$effect(() => {
		const key = resourceKey;
		diagnosticLog("detail.mount", { key });
		return () => diagnosticLog("detail.unmount", { key });
	});

	const detailsQuery = createQuery<ResourceDetailsFull>(() =>
		resourceDetailsQuery(client, readSpec, detailsEnabled),
	);
	const eventsUid = $derived.by(() => {
		const uid = detailsQuery.data?.metadata?.uid;
		return String(uid) === uid ? uid : undefined;
	});
	// Events are matched by UID, so wait for the details read that supplies it.
	const eventsReady = $derived(
		!detailsEnabled || detailsQuery.isSuccess || detailsQuery.isError,
	);
	const eventsQuery = createQuery<ResourceEventSummary[]>(() => ({
		...resourceEventsQuery(client, readSpec, eventsUid, eventsEnabled && eventsReady, queryClient),
		refetchInterval: !watches.eventsConnected
			? (query) => query.state.status === "success" ? RESOURCE_RECOVERY_INTERVAL : false
			: false,
	}));

	const detailResource = $derived(detailsQuery.data?.summary ?? resource);
	const conditionRows = $derived(getConditionRows(detailsQuery.data?.status));
	const containerRows = $derived(getContainerStatusRows(detailsQuery.data?.status, detailResource.kind));
	const containerOptions = $derived.by(() => {
		const regularContainers = containerRows.filter((container) => container.type !== "init");
		return (regularContainers.length > 0 ? regularContainers : containerRows).map(
			(container) => container.name,
		);
	});
	const metadataRows = $derived(formatObjectRows(detailsQuery.data?.metadata ?? {}));
	const curatedMetadata = $derived(
		buildCuratedMetadata(detailsQuery.data?.metadata ?? {}, detailResource),
	);
	const visibleMetadataLabels = $derived(
		visibleMetadataBadges(curatedMetadata.labels, metadataLabelsExpanded),
	);
	const visibleMetadataAnnotations = $derived(
		visibleMetadataBadges(curatedMetadata.annotations, metadataAnnotationsExpanded),
	);
	const statusRows = $derived(formatObjectRows(detailsQuery.data?.status ?? {}));
	const resourceYaml = $derived(detailsQuery.data?.yaml ?? "");
	const sortedEvents = $derived(sortIncidentEvents(eventsQuery.data ?? []));
	const signalContainers = $derived(
		detailResource.kind === "Pod" &&
			(detailsQuery.data || (detailsQuery.isPending && !detailsQuery.isError))
			? containerRows
			: undefined,
	);
	const incidentSignals = $derived(
		buildIncidentSignals(
			detailResource,
			conditionRows,
			sortedEvents,
			signalContainers,
		),
	);
	const topIncidentSignals = $derived(incidentSignals.slice(0, 3));
	const incidentTimeline = $derived(
		activeTab === "details" ? buildIncidentTimeline({
			resource: detailResource,
			conditions: conditionRows,
			events: sortedEvents,
			containers: signalContainers,
			logLines: parsedLogLines,
		}) : [],
	);

	$effect(() => {
		const requestedTab = initialPathState?.activeTab ?? null;
		if (!requestedTab || requestedTab === appliedInitialActiveTab) return;
		appliedInitialActiveTab = requestedTab;
		if (isDetailTabAvailable(requestedTab)) {
			activeTab = requestedTab;
		}
	});

	$effect(() => {
		if (!isDetailTabAvailable(activeTab)) {
			activeTab = "details";
		}
	});

	$effect(() => {
		onPathStateChange({
			activeTab,
			metadataLabelsExpanded,
			metadataAnnotationsExpanded,
			selectedContainer,
			logFilter,
			logWrapLines,
			logLatestFirst,
			yamlViewMode,
			yamlEncoding,
			yamlShowFullDiff,
		});
	});

	function formatLogTime(timestamp: string | undefined): string {
		return formatExactTimeOnly(timestamp, timestampTimezone) ?? "-";
	}

	function formatFullTimestamp(timestamp: string | null | undefined): string {
		return formatExactTimestamp(timestamp, timestampTimezone, "millisecond") ?? timestamp ?? "-";
	}

	function formatEventExactTime(event: ResourceEventSummary): string {
		return formatExactTimestamp(event.lastSeenAt, timestampTimezone, "millisecond") ?? event.lastSeen;
	}

	function formatEventCompactTime(event: ResourceEventSummary): string {
		return formatExactTimeOnly(event.lastSeenAt, timestampTimezone, "second") ?? event.lastSeen;
	}

	function tabChanged(value: string) {
		const nextTab = DETAIL_TABS.find((tab) => tab === value);
		if (!nextTab) return;
		if (isDetailTabAvailable(nextTab)) {
			diagnosticLog("detail.tab.click", { key: resourceKey, tab: nextTab });
			activeTab = nextTab;
		}
	}

	function isDetailTabAvailable(tab: DetailTab): boolean {
		if (tab === "logs") return canShowLogs;
		if (tab === "exec") return canShowExec;
		if (tab === "portForward") return canShowPortForward;
		if (tab === "revisions") return canShowRevisions;
		if (tab === "operations") return true;
		if (tab === "argo") return isArgoApplication;
		return true;
	}
</script>

<Tabs value={activeTab} onValueChange={tabChanged} class="h-full min-h-0 gap-3">
	<TabsList variant="line" class="flex-wrap justify-start">
		<TabsTrigger value="details">Details</TabsTrigger>
		<TabsTrigger value="events">Events</TabsTrigger>
		{#if canShowLogs}<TabsTrigger value="logs">Logs</TabsTrigger>{/if}
		{#if canShowExec}<TabsTrigger value="exec">Exec</TabsTrigger>{/if}
		{#if canShowPortForward}<TabsTrigger value="portForward">Forward</TabsTrigger>{/if}
		{#if canShowRevisions}<TabsTrigger value="revisions">Revisions</TabsTrigger>{/if}
		{#if isArgoApplication}<TabsTrigger value="argo">Argo CD</TabsTrigger>{/if}
		<TabsTrigger value="operations">Actions</TabsTrigger>
		<TabsTrigger value="yaml">YAML</TabsTrigger>
	</TabsList>

	{#if watches.resourceError}
		<p class="text-muted-foreground text-xs">
			Realtime watch failed: {watches.resourceError}. Showing the last loaded data.
		</p>
	{/if}
	{#if showEventsWarnings && watches.eventsError}
		<p class="text-muted-foreground text-xs">
			Events watch failed: {watches.eventsError}
		</p>
	{/if}
	{#if showEventsWarnings && eventsQuery.isRefetchError}
		<p class="text-muted-foreground text-xs">
			Events refresh failed: {messageFromError(eventsQuery.error)}
		</p>
	{/if}

	{#if visitedTabs.has("details")}<DetailsTab
		{resourceKey}
		{detailsQuery}
		{eventsQuery}
		{detailResource}
		{conditionRows}
		{containerRows}
		{metadataRows}
		{statusRows}
		{curatedMetadata}
		{visibleMetadataLabels}
		{visibleMetadataAnnotations}
		{topIncidentSignals}
		{incidentTimeline}
		{onOpenHelmRelease}
		requiredPermission={detailReadPermission}
		bind:metadataLabelsExpanded
		bind:metadataAnnotationsExpanded
		{formatFullTimestamp}
	/>{/if}

	{#if visitedTabs.has("yaml")}
		<TabsContent value="yaml" class="flex min-h-0 flex-col">
		<DeferredSurface load={loadYamlPane} label="YAML" showStatus={activeTab === "yaml"}>
			{#snippet children(ResourceYamlPane)}
				<ResourceYamlPane
					{client}
					{resource}
					{dynamicKind}
					{kubeconfigSourceKey}
					detailsYaml={resourceYaml}
					{detailsEnabled}
					active={activeTab === "yaml"}
					refreshVersion={resourceRefreshVersion}
					bind:yamlViewMode
					bind:yamlEncoding
					bind:yamlShowFullDiff
				/>
			{/snippet}
		</DeferredSurface>
		</TabsContent>
	{/if}

	{#if visitedTabs.has("events")}<EventsTab
		{eventsQuery}
		{sortedEvents}
		{formatEventExactTime}
		{formatEventCompactTime}
	/>{/if}

	{#if canShowLogs && visitedTabs.has("logs")}
		<ResourceLogsPane
			{client}
			{resource}
			{kubeconfigSourceKey}
			{isPod}
			{containerOptions}
			active={activeTab === "logs"}
			bind:selectedContainer
			bind:logFilter
			bind:logWrapLines
			bind:logLatestFirst
			bind:parsedLogLines
			{formatFullTimestamp}
			{formatLogTime}
		/>
	{/if}

	{#if canShowExec}<TabsContent value="exec" class="flex min-h-0 flex-col">
		{#if ExecTabComponent && !execModuleLoading.active}
			<ExecTabComponent
				{client}
				{resource}
				containers={containerRows}
				{selectedContainer}
				{kubeconfigSourceKey}
				active={activeTab === "exec"}
				onShow={() => (activeTab = "exec")}
			/>
		{:else if activeTab === "exec" && execTabLoadError}
			<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
				<span>Failed to load Exec.</span>
				<Button type="button" variant="outline" size="sm" onclick={retryExecTab}>Retry</Button>
			</div>
		{:else if activeTab === "exec" && execModuleLoading.shown}
			<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
				<Spinner />
				<span>Loading Exec</span>
			</div>
		{/if}
	</TabsContent>{/if}

	{#if canShowPortForward && visitedTabs.has("portForward")}<TabsContent value="portForward">
		<PortForwardTab
			{client}
			{resource}
			{kubeconfigSourceKey}
			yaml={resourceYaml}
			active={activeTab === "portForward"}
		/>
	</TabsContent>{/if}
	{#if canShowRevisions && visitedTabs.has("revisions")}
		<DeploymentRevisionsTab
			{client}
			{resource}
			{kubeconfigSourceKey}
			active={activeTab === "revisions"}
		/>
	{/if}
	{#if isArgoApplication && visitedTabs.has("argo")}
		<TabsContent value="argo" class="min-h-0">
			<DeferredSurface load={loadArgoPane} label="Argo CD">
				{#snippet children(ArgoApplicationDetails)}
					<ArgoApplicationDetails
							resourceSummary={resource}
							applicationSummary={argoApplicationSummary}
							applicationDetails={detailsQuery.data ?? null}
							{workspaceReadContext}
							active={activeTab === "argo"}
						/>
				{/snippet}
			</DeferredSurface>
		</TabsContent>
	{/if}
	<TabsContent value="operations">
		{#if visitedTabs.has("operations")}<ResourceActionsTab {client} resource={detailResource} {kubeconfigSourceKey} />{/if}
	</TabsContent>
</Tabs>

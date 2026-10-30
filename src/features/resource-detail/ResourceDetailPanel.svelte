<script module lang="ts">
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
	import { SvelteSet } from "svelte/reactivity";
	import { Button, Spinner, Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/svelte";
	import {
		formatExactTimeOnly,
		formatExactTimestamp,
	} from "@/components/timestamp-format";
	import { diagnosticLog } from "@/lib/diagnostics";
	import {
		closeStreamChannel,
		createStreamChannel,
		startResourceEventWatch,
		startResourceWatch,
		stopStream,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		ArgoApplicationSummary,
		ResourceDetailsFull,
		ResourceEventSummary,
		ResourceSummary,
		StreamMessage,
		YamlEncoding,
		YamlViewMode,
	} from "@/lib/types";
	import type { PathStateResourceDetailState } from "@/lib/path-state";
	import { argoApplicationReads } from "@/lib/argo-application-inspection";
	import { createLiveReadRefresh } from "@/lib/live-view-refresh";
	import { queryMatchers, type QueryMatch } from "@/lib/queryKeys";
	import type { WorkspaceReadContext } from "@/lib/workspaceReadContext";
	import {
		getSettingsSnapshot,
		settingsStore,
	} from "@/lib/settings-store";
	import {
		buildIncidentSignals,
		getConditionRows,
		getContainerStatusRows,
		isCleanCompletedContainer,
		type ContainerStatusRow,
	} from "./helpers";
	import {
		buildResourceDetailReadSpec,
		resourceDetailsQuery,
		resourceEventsQuery,
	} from "./resourceDetailReadSpec";
	import ArgoApplicationDetails from "@/features/gitops/ArgoApplicationDetails.svelte";
	import { CHIP_BADGE_STYLES, type ChipVariant } from "./constants";
	import DetailsTab from "./DetailsTab.svelte";
	import DeploymentRevisionsTab from "./DeploymentRevisionsTab.svelte";
	import EventsTab from "./EventsTab.svelte";
	import PortForwardTab from "./PortForwardTab.svelte";
	import ResourceActionsTab from "./ResourceActionsTab.svelte";
	import ResourceLogsPane from "./ResourceLogsPane.svelte";
	import ResourceYamlPane from "./ResourceYamlPane.svelte";
	import { sortIncidentEvents } from "./incident-events";
	import {
		buildIncidentTimeline,
		type IncidentTimelineTone,
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
		onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
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
	let realtimeWatchError = $state<string | null>(null);
	let realtimeEventsWatchError = $state<string | null>(null);
	let yamlShowFullDiff = $state(initialDetailState?.yamlShowFullDiff ?? false);
	let resourceRefreshVersion = $state(0);
	let ExecTabComponent = $state<typeof import("./ExecTab.svelte").default | null>(null);
	let execTabLoadError = $state<unknown>(null);

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
	const detailsQueryKey = $derived(readSpec.detailsQueryKey);
	const eventsQueryKey = $derived(readSpec.eventsQueryKey);

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
	const eventsQuery = createQuery<ResourceEventSummary[]>(() =>
		resourceEventsQuery(client, readSpec, eventsUid, eventsEnabled && eventsReady),
	);

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

	$effect(() => {
		if (!detailsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;
		const watchKey = readSpec.resourceWatchKey;
		let needsCatchUp = false;
		const details = queryMatchers.exact(detailsQueryKey);
		const argoReads = argoApplicationReadsMatch;
		const reads: QueryMatch = argoReads ? (query) => details(query) || argoReads(query) : details;
		const refresh = createLiveReadRefresh(queryClient, reads, () => {
			if (activeTab === "yaml") resourceRefreshVersion += 1;
		});
		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "error") {
				realtimeWatchError = event.message;
				needsCatchUp = true;
				return;
			}
			if (event.type === "status") {
				if (event.status === "connected") {
					realtimeWatchError = null;
					if (needsCatchUp) {
						needsCatchUp = false;
						refresh.notify();
					}
				} else needsCatchUp = true;
				return;
			}
			if (event.type !== "resourceChanged") return;
			if (event.target.name && event.target.name !== resource.name) return;
			if (
				resource.namespace &&
				event.target.namespace &&
				event.target.namespace !== resource.namespace
			) {
				return;
			}
			refresh.notify();
		}, client);
		void startResourceWatch(
			client,
			resource.cluster,
			[watchKey],
			channel,
			kubeconfigSourceKey,
		)
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((cause: unknown) => {
				if (cancelled) return;
				realtimeWatchError = cause instanceof Error ? cause.message : String(cause);
			});
		return () => {
			cancelled = true;
			refresh.dispose();
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});

	$effect(() => {
		if (!eventsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;
		const refresh = createLiveReadRefresh(queryClient, queryMatchers.prefix(eventsQueryKey));
		let needsCatchUp = false;
		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "error") {
				realtimeEventsWatchError = event.message;
				needsCatchUp = true;
				return;
			}
			if (event.type === "status") {
				if (event.status === "connected") {
					realtimeEventsWatchError = null;
					if (needsCatchUp) {
						needsCatchUp = false;
						refresh.notify();
					}
				} else needsCatchUp = true;
				return;
			}
			if (event.type === "resourceEventsChanged") refresh.notify();
		}, client);
		void startResourceEventWatch(
			client,
			readSpec.eventWatch.cluster,
			readSpec.eventWatch.kind,
			readSpec.eventWatch.name,
			readSpec.eventWatch.namespace,
			channel,
			kubeconfigSourceKey,
		)
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((cause: unknown) => {
				if (cancelled) return;
				realtimeEventsWatchError = cause instanceof Error ? cause.message : String(cause);
			});
		return () => {
			cancelled = true;
			refresh.dispose();
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
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

	function hashMetadataKeyToHue(key: string): number {
		let hash = 0;
		for (let index = 0; index < key.length; index += 1) {
			hash = (hash * 31 + key.charCodeAt(index)) >>> 0;
		}
		return hash % 360;
	}

	function metadataBadgeStyle(key: string): string {
		const hue = hashMetadataKeyToHue(key);
		return [
			`background-color: hsl(${hue} 70% 22% / 0.38)`,
			`border-color: hsl(${hue} 72% 52% / 0.46)`,
			`color: hsl(${hue} 88% 78%)`,
		].join("; ");
	}

	function incidentToneClass(tone: IncidentTimelineTone | "success"): string {
		switch (tone) {
			case "error":
				return "border-l-red-500 bg-red-500/5";
			case "warning":
				return "border-l-amber-500 bg-amber-500/5";
			case "info":
				return "border-l-sky-500 bg-sky-500/5";
			case "success":
				return "border-l-emerald-500 bg-emerald-500/5";
			case "neutral":
				return "border-l-muted bg-background/50";
		}
	}

	function toneBadgeVariant(tone: ChipVariant): "secondary" | "destructive" | "outline" {
		return CHIP_BADGE_STYLES[tone].variant;
	}

	function toneBadgeClass(tone: ChipVariant, extra = ""): string {
		return `${CHIP_BADGE_STYLES[tone].className} ${extra}`.trim();
	}

	function compactToneBadgeClass(tone: ChipVariant): string {
		return toneBadgeClass(tone, "rounded-full px-2 py-0 text-[0.6875rem] shadow-none");
	}

	function restartsTone(restarts: number | undefined): ChipVariant {
		return restarts !== undefined && restarts > 0 ? "warning" : "neutral";
	}

	function containerTone(container: ContainerStatusRow): ChipVariant {
		if (isCleanCompletedContainer(container)) return "neutral";
		if (container.exitCode !== undefined && container.exitCode !== 0) return "error";
		if (
			container.state === "waiting" ||
			(container.lastExitCode !== undefined && container.lastExitCode !== 0)
		) {
			return "warning";
		}
		if (container.ready === false) return "error";
		if (container.ready === true || container.state === "running") return "success";
		return "neutral";
	}

	function containerReadyLabel(container: ContainerStatusRow): string {
		if (isCleanCompletedContainer(container)) return "Completed";
		if (container.ready === undefined) return "Not reported";
		return container.ready ? "Yes" : "No";
	}

	function containerStateLabel(container: ContainerStatusRow): string {
		const state = container.state ?? "-";
		const reason = container.reason ?? container.lastReason;
		return reason && state !== "-" ? `${state} - ${reason}` : (reason ?? state);
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

	{#if realtimeWatchError || realtimeEventsWatchError}
		<p class="text-muted-foreground text-xs">
			Realtime watch failed{#if realtimeWatchError}: {realtimeWatchError}{/if}{#if realtimeEventsWatchError}{#if realtimeWatchError} · {:else}: {/if}events: {realtimeEventsWatchError}{/if}. Showing the last loaded data.
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
		{toneBadgeVariant}
		{compactToneBadgeClass}
		{restartsTone}
		{incidentToneClass}
		{formatFullTimestamp}
		{metadataBadgeStyle}
		{containerTone}
		{containerReadyLabel}
		{containerStateLabel}
	/>{/if}

	{#if visitedTabs.has("yaml")}<ResourceYamlPane
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
	/>{/if}

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
		{#if ExecTabComponent}
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
		{:else if activeTab === "exec"}
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
			<ArgoApplicationDetails
					resourceSummary={resource}
					applicationSummary={argoApplicationSummary}
					applicationDetails={detailsQuery.data ?? null}
					{workspaceReadContext}
					active={activeTab === "argo"}
				/>
		</TabsContent>
	{/if}
	<TabsContent value="operations">
		{#if visitedTabs.has("operations")}<ResourceActionsTab {client} resource={detailResource} {kubeconfigSourceKey} />{/if}
	</TabsContent>
</Tabs>

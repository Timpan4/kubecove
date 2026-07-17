<script module lang="ts">
	let resourceYamlPaneImport: Promise<typeof import("./ResourceYamlPane.svelte")> | undefined;
	let execTabImport: Promise<typeof import("./ExecTab.svelte")> | undefined;

	function loadResourceYamlPane() {
		return (resourceYamlPaneImport ??= import("./ResourceYamlPane.svelte"));
	}

	function loadExecTab() {
		return (execTabImport ??= import("./ExecTab.svelte"));
	}

	function retryResourceYamlPaneLoad() {
		resourceYamlPaneImport = undefined;
		return loadResourceYamlPane();
	}

	function retryExecTabLoad() {
		execTabImport = undefined;
		return loadExecTab();
	}
</script>

<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { Button, Spinner, Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/svelte";
	import {
		formatExactTimeOnly,
		formatExactTimestamp,
	} from "@/components/timestamp-format";
	import { createCancellableRequest, createCancelScope } from "@/lib/cancellable-loads";
	import { diagnosticLog, diagnosticResultSummary } from "@/lib/diagnostics";
	import { withForegroundLoad } from "@/lib/foreground-loading";
	import {
		cancelBackendRequests,
		closeStreamChannel,
		createStreamChannel,
		getDynamicResourceDetails,
		getResourceDetails,
		isAppError,
		listResourceEvents,
		startResourceEventWatch,
		startResourceWatch,
		stopStream,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		ResourceDetailsFull,
		ResourceEventSummary,
		ResourceSummary,
		StreamMessage,
		WatchResourceKey,
		YamlEncoding,
		YamlViewMode,
	} from "@/lib/types";
	import type { PathStateResourceDetailState } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import {
		getSettingsSnapshot,
		settingsStore,
	} from "@/lib/settings-store";
	import {
		buildIncidentSignals,
		dynamicResourceKindFromSummary,
		getConditionRows,
		getContainerStatusRows,
		isCleanCompletedContainer,
		shouldFetchResourceDetails,
		shouldFetchResourceEvents,
		type ContainerStatusRow,
	} from "./helpers";
	import { CHIP_BADGE_STYLES, type ChipVariant } from "./constants";
	import DetailsTab from "./DetailsTab.svelte";
	import DeploymentRevisionsTab from "./DeploymentRevisionsTab.svelte";
	import EventsTab from "./EventsTab.svelte";
	import PortForwardTab from "./PortForwardTab.svelte";
	import OperationsTab from "./OperationsTab.svelte";
	import ResourceLogsPane from "./ResourceLogsPane.svelte";
	import { sortIncidentEvents } from "./incident-events";
	import {
		buildIncidentTimeline,
		type IncidentTimelineTone,
	} from "./incident-timeline";
	import type { ParsedLogLine } from "./log-helpers";
	import {
		buildCuratedMetadata,
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
		| "operations";
	type DetailFetchKind = "details" | "events";

	let {
		client,
		resource,
		kubeconfigSourceKey,
		onOpenHelmRelease,
		initialPathState = null,
		onPathStateChange = () => {},
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
		onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
		initialPathState?: PathStateResourceDetailState | null;
		onPathStateChange?: (state: PathStateResourceDetailState) => void;
	} = $props();

	const queryClient = useQueryClient();
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
	let logAutoFollow = $state(initialDetailState?.logAutoFollow ?? true);
	let parsedLogLines = $state<ParsedLogLine[]>([]);
	let yamlShowFullDiff = $state(initialDetailState?.yamlShowFullDiff ?? false);
	let resourceRefreshVersion = $state(0);
	let ResourceYamlPaneComponent = $state<typeof import("./ResourceYamlPane.svelte").default | null>(null);
	let resourceYamlPaneLoadError = $state<unknown>(null);
	let ExecTabComponent = $state<typeof import("./ExecTab.svelte").default | null>(null);
	let execTabLoadError = $state<unknown>(null);

	const resourceKey = $derived(
		`${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`,
	);
	const dynamicKind = $derived(dynamicResourceKindFromSummary(resource));
	const dynamicKindKey = $derived(
		dynamicKind
			? `${dynamicKind.group}/${dynamicKind.version}/${dynamicKind.kind}/${dynamicKind.plural}/${dynamicKind.namespaced}`
			: "",
	);
	const detailsEnabled = $derived(shouldFetchResourceDetails(resource));
	const eventsEnabled = $derived(shouldFetchResourceEvents(resource));
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

	$effect(() => {
		if (activeTab !== "yaml" || ResourceYamlPaneComponent || resourceYamlPaneLoadError) return;
		void loadResourceYamlPane()
			.then((module) => {
				ResourceYamlPaneComponent = module.default;
			})
			.catch((error: unknown) => {
				resourceYamlPaneLoadError = error;
			});
	});

	$effect(() => {
		if (activeTab !== "exec" || ExecTabComponent || execTabLoadError) return;
		void loadExecTab()
			.then((module) => {
				ExecTabComponent = module.default;
			})
			.catch((error: unknown) => {
				execTabLoadError = error;
			});
	});

	function retryResourceYamlPane() {
		resourceYamlPaneLoadError = null;
		void retryResourceYamlPaneLoad()
			.then((module) => {
				ResourceYamlPaneComponent = module.default;
			})
			.catch((error: unknown) => {
				resourceYamlPaneLoadError = error;
			});
	}

	function retryExecTab() {
		execTabLoadError = null;
		void retryExecTabLoad()
			.then((module) => {
				ExecTabComponent = module.default;
			})
			.catch((error: unknown) => {
				execTabLoadError = error;
			});
	}
	const detailsQueryKey = $derived(
		queryKeys.resourceDetails(resource, dynamicKindKey, kubeconfigSourceKey, yamlViewMode, yamlEncoding),
	);
	const eventsQueryKey = $derived(queryKeys.resourceEvents(resource, kubeconfigSourceKey));
	const detailsCancelScope = $derived(createCancelScope("resource-details", detailsQueryKey));
	const eventsCancelScope = $derived(createCancelScope("resource-events", eventsQueryKey));
	const pendingCancelTimers = new Map<string, ReturnType<typeof setTimeout>>();

	$effect(() => {
		const key = resourceKey;
		diagnosticLog("detail.mount", { key });
		return () => diagnosticLog("detail.unmount", { key });
	});

	async function runDetailFetch<T>(
		kind: DetailFetchKind,
		loadLabel: string,
		task: () => Promise<T>,
	): Promise<T> {
		const started = performance.now();
		diagnosticLog(`detail.${kind}.fetch.start`, { key: resourceKey });
		const result = await withForegroundLoad(loadLabel, task);
		diagnosticLog(`detail.${kind}.fetch.done`, {
			key: resourceKey,
			ms: Math.round(performance.now() - started),
			result: diagnosticResultSummary(result),
		});
		return result;
	}

	function cancelPendingBackendScope(cancelScope: string) {
		const timer = pendingCancelTimers.get(cancelScope);
		if (!timer) return;
		clearTimeout(timer);
		pendingCancelTimers.delete(cancelScope);
	}

	function scheduleBackendScopeCancel(
		cancelScope: string,
		queryKey: readonly unknown[],
		event: string,
	) {
		cancelPendingBackendScope(cancelScope);
		const timer = setTimeout(() => {
			pendingCancelTimers.delete(cancelScope);
			const query = queryClient.getQueryCache().find({ queryKey, exact: true });
			if ((query?.getObserversCount() ?? 0) > 0) return;
			void queryClient.cancelQueries({ queryKey, exact: true });
			void cancelBackendRequests(client, cancelScope)
				.then((result) => {
					if (result.cancelled > 0) diagnosticLog(event, { cancelled: result.cancelled });
				})
				.catch((error: unknown) => {
					diagnosticLog(`${event}.error`, {
						error: error instanceof Error ? error.message : String(error),
					});
				});
		}, 0);
		pendingCancelTimers.set(cancelScope, timer);
	}

	$effect(() => {
		const currentDetailsCancelScope = detailsCancelScope;
		const currentDetailsQueryKey = detailsQueryKey;
		const currentEventsCancelScope = eventsCancelScope;
		const currentEventsQueryKey = eventsQueryKey;
		cancelPendingBackendScope(currentDetailsCancelScope);
		cancelPendingBackendScope(currentEventsCancelScope);
		return () => {
			scheduleBackendScopeCancel(
				currentDetailsCancelScope,
				currentDetailsQueryKey,
				"detail.details.cancel",
			);
			scheduleBackendScopeCancel(
				currentEventsCancelScope,
				currentEventsQueryKey,
				"detail.events.cancel",
			);
		};
	});

	const detailsQuery = createQuery<ResourceDetailsFull>(() => ({
		queryKey: detailsQueryKey,
		queryFn: async () => {
			try {
				return await runDetailFetch("details", "resource-details", () =>
					dynamicKind
						? getDynamicResourceDetails(
							client,
							resource.cluster,
							dynamicKind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigSourceKey,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						)
						: getResourceDetails(
							client,
							resource.cluster,
							resource.kind,
							resource.name,
							resource.namespace ?? undefined,
							kubeconfigSourceKey,
							yamlViewMode,
							yamlEncoding,
							createCancellableRequest(detailsCancelScope, "details"),
						),
				);
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("detail.details.cancel", { key: resourceKey });
				}
				throw error;
			}
		},
		enabled: detailsEnabled,
		retry: false,
		staleTime: 30_000,
	}));
	const eventsQuery = createQuery<ResourceEventSummary[]>(() => ({
		queryKey: eventsQueryKey,
		queryFn: async () => {
			try {
				return await runDetailFetch("events", "resource-events", () =>
					listResourceEvents(
						client,
						resource.cluster,
						resource.kind,
						resource.name,
						resource.namespace ?? undefined,
						kubeconfigSourceKey,
						createCancellableRequest(eventsCancelScope, "events"),
					),
				);
			} catch (error) {
				if (isAppError(error) && error.kind === "cancelled") {
					diagnosticLog("detail.events.cancel", { key: resourceKey });
				}
				throw error;
			}
		},
		enabled: eventsEnabled,
		retry: false,
		staleTime: 30_000,
	}));

	const detailResource = $derived(detailsQuery.data?.summary ?? resource);
	const conditionRows = $derived(getConditionRows(detailsQuery.data?.status));
	const containerRows = $derived(getContainerStatusRows(detailsQuery.data?.status));
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
		buildIncidentTimeline({
			resource: detailResource,
			conditions: conditionRows,
			events: sortedEvents,
			containers: signalContainers,
			logLines: parsedLogLines,
		}),
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
			logAutoFollow,
			yamlViewMode,
			yamlEncoding,
			yamlShowFullDiff,
		});
	});

	$effect(() => {
		if (!detailsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;
		let debounce: ReturnType<typeof setTimeout> | null = null;
		const watchKey: WatchResourceKey = dynamicKind
			? {
					resourceKind: {
						kind: dynamicKind.kind,
						group: dynamicKind.group,
						version: dynamicKind.version,
						apiVersion: dynamicKind.apiVersion,
						plural: dynamicKind.plural,
						namespaced: dynamicKind.namespaced,
					},
					namespace: resource.namespace ?? undefined,
				}
			: {
					resourceKind: { kind: resource.kind },
					namespace: resource.namespace ?? undefined,
				};
		const invalidateSoon = () => {
			if (debounce) clearTimeout(debounce);
			debounce = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey: detailsQueryKey });
				if (activeTab === "yaml") {
					resourceRefreshVersion += 1;
				}
			}, 250);
		};
		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled || event.type !== "resourceChanged") return;
			if (event.target.name && event.target.name !== resource.name) return;
			if (
				resource.namespace &&
				event.target.namespace &&
				event.target.namespace !== resource.namespace
			) {
				return;
			}
			invalidateSoon();
		});
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
			.catch(() => {});
		return () => {
			cancelled = true;
			if (debounce) clearTimeout(debounce);
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});

	$effect(() => {
		if (!eventsEnabled) return;
		let cancelled = false;
		let streamId: string | null = null;
		let debounce: ReturnType<typeof setTimeout> | null = null;
		const invalidateSoon = () => {
			if (debounce) clearTimeout(debounce);
			debounce = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey: eventsQueryKey });
			}, 250);
		};
		const channel = createStreamChannel((event: StreamMessage) => {
			if (!cancelled && event.type === "resourceEventsChanged") invalidateSoon();
		});
		void startResourceEventWatch(
			client,
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace ?? undefined,
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
			.catch(() => {});
		return () => {
			cancelled = true;
			if (debounce) clearTimeout(debounce);
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});

	function formatUnknown(value: unknown): string {
		if (value === undefined || value === null) return "-";
		if (typeof value === "string") return value;
		if (typeof value === "number" || typeof value === "boolean") return String(value);
		return JSON.stringify(value, null, 2);
	}

	function formatObjectRows(value: Record<string, unknown>) {
		return Object.entries(value).map(([key, item]) => ({ key, value: formatUnknown(item) }));
	}

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

	function detailStatusLabel(summary: ResourceSummary): string | undefined {
		return summary.status ?? summary.health;
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
		if (container.ready === undefined) return "-";
		return container.ready ? "Yes" : "No";
	}

	function containerStateLabel(container: ContainerStatusRow): string {
		const state = container.state ?? "-";
		const reason = container.reason ?? container.lastReason;
		return reason && state !== "-" ? `${state} - ${reason}` : (reason ?? state);
	}

	function tabChanged(value: string) {
		const nextTab = value as DetailTab;
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
		<TabsTrigger value="operations">Actions</TabsTrigger>
		<TabsTrigger value="yaml">YAML</TabsTrigger>
	</TabsList>

	<DetailsTab
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
		bind:metadataLabelsExpanded
		bind:metadataAnnotationsExpanded
		{detailStatusLabel}
		{toneBadgeVariant}
		{compactToneBadgeClass}
		{restartsTone}
		{incidentToneClass}
		{formatFullTimestamp}
		{metadataBadgeStyle}
		{containerTone}
		{containerReadyLabel}
		{containerStateLabel}
	/>

	{#if ResourceYamlPaneComponent}
		<ResourceYamlPaneComponent
			{client}
			{resource}
			{dynamicKind}
			{kubeconfigSourceKey}
			detailsYaml={resourceYaml}
			{detailsQueryKey}
			{detailsEnabled}
			active={activeTab === "yaml"}
			refreshVersion={resourceRefreshVersion}
			bind:yamlViewMode
			bind:yamlEncoding
			bind:yamlShowFullDiff
		/>
	{:else if activeTab === "yaml"}
		<TabsContent value="yaml" class="min-h-0">
			{#if resourceYamlPaneLoadError}
				<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
					<span>Failed to load YAML.</span>
					<Button type="button" variant="outline" size="sm" onclick={retryResourceYamlPane}>Retry</Button>
				</div>
			{:else}
				<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
					<Spinner />
					<span>Loading YAML</span>
				</div>
			{/if}
		</TabsContent>
	{/if}

	<EventsTab
		{eventsQuery}
		{sortedEvents}
		{formatEventExactTime}
		{formatEventCompactTime}
	/>

	{#if canShowLogs}
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
			bind:logAutoFollow
			bind:logLatestFirst
			bind:parsedLogLines
			{formatFullTimestamp}
			{formatLogTime}
		/>
	{/if}

	{#if canShowExec}<TabsContent value="exec">
		{#if ExecTabComponent}
			<ExecTabComponent
				{client}
				{resource}
				containers={containerRows}
				bind:selectedContainer
				{kubeconfigSourceKey}
				active={activeTab === "exec"}
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

	{#if canShowPortForward}<TabsContent value="portForward">
		<PortForwardTab
			{client}
			{resource}
			{kubeconfigSourceKey}
			yaml={resourceYaml}
			active={activeTab === "portForward"}
		/>
	</TabsContent>{/if}
	{#if canShowRevisions}
		<DeploymentRevisionsTab
			{client}
			{resource}
			{kubeconfigSourceKey}
			active={activeTab === "revisions"}
		/>
	{/if}
	<TabsContent value="operations">
		<OperationsTab {client} resource={detailResource} {kubeconfigSourceKey} />
	</TabsContent>
</Tabs>

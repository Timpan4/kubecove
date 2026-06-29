<script lang="ts">
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/svelte";
	import type { Diagnostic } from "@codemirror/lint";
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
		applyYaml,
		getDynamicResourceDetails,
		getResourceDetails,
		getResourceYaml,
		isAppError,
		listResourceEvents,
		lintKubernetesYaml,
		prepareYamlApply,
		startPodLogStream,
		startResourceEventWatch,
		startResourceWatch,
		stopStream,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		PodLogStreamRequest,
		ResourceDetailsFull,
		ResourceEventSummary,
		ResourceSummary,
		StreamMessage,
		WatchResourceKey,
		YamlApplyPreview,
		YamlEncoding,
		YamlViewMode,
		KubernetesYamlLintDiagnostic,
		KubernetesYamlLintStatusNote,
	} from "@/lib/types";
	import type { PathStateResourceDetailState } from "@/lib/path-state";
	import { queryKeys } from "@/lib/queryKeys";
	import { formatYamlDocument } from "@/lib/yamlFormat";
	import {
		getSettingsSnapshot,
		settingsStore,
	} from "@/lib/settings-store";
	import {
		buildIncidentSignals,
		dynamicResourceKindFromSummary,
		getConditionRows,
		getContainerStatusRows,
		getErrorMessage,
		isCleanCompletedContainer,
		shouldFetchResourceDetails,
		shouldFetchResourceEvents,
		type ContainerStatusRow,
	} from "./helpers";
	import { CHIP_BADGE_STYLES, type ChipVariant } from "./constants";
	import DetailsTab from "./DetailsTab.svelte";
	import EventsTab from "./EventsTab.svelte";
	import ExecTab from "./ExecTab.svelte";
	import LogsTab from "./LogsTab.svelte";
	import PortForwardTab from "./PortForwardTab.svelte";
	import YamlTab from "./YamlTab.svelte";
	import {
		buildYamlDryRunDiff,
		findYamlFieldRange,
	} from "./yamlTabDiff";
	import { sortIncidentEvents } from "./incident-events";
	import {
		buildIncidentTimeline,
		type IncidentTimelineTone,
	} from "./incident-timeline";
	import { orderedLogLines } from "./log-helpers";
	import {
		buildCuratedMetadata,
		visibleMetadataBadges,
	} from "./metadata-details";
	import {
		buildYamlApplyRequest as createYamlApplyRequest,
		isYamlApplyDisabled,
		resolveYamlForceConflicts,
		yamlAppliedMessage as formatYamlAppliedMessage,
		yamlApplyTargetLabel,
	} from "./yamlApplyModel";

	type DetailTab = "details" | "yaml" | "events" | "logs" | "exec" | "portForward";
	type DetailFetchKind = "details" | "yaml" | "events";

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
	let metadataLabelsExpanded = $state(initialDetailState?.metadataLabelsExpanded ?? false);
	let metadataAnnotationsExpanded = $state(
		initialDetailState?.metadataAnnotationsExpanded ?? false,
	);
	let selectedContainer = $state(initialDetailState?.selectedContainer ?? "");
	let logLines = $state<string[]>([]);
	let logStatus = $state("idle");
	let logMessage = $state("Log stream idle");
	let logError = $state<unknown>(null);
	let logFilter = $state(initialDetailState?.logFilter ?? "");
	let logWrapLines = $state(initialDetailState?.logWrapLines ?? true);
	let logLatestFirst = $state(initialDetailState?.logLatestFirst ?? false);
	let logAutoFollow = $state(initialDetailState?.logAutoFollow ?? true);
	let logViewport = $state<HTMLElement | null>(null);
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
	let yamlShowFullDiff = $state(initialDetailState?.yamlShowFullDiff ?? false);

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
	const yamlEnabled = $derived(detailsEnabled && activeTab === "yaml");
	const isPod = $derived(resource.kind === "Pod" && Boolean(resource.namespace));
	const canShowLogs = $derived(isPod);
	const canShowExec = $derived(isPod);
	const canShowPortForward = $derived(
		(resource.kind === "Pod" || resource.kind === "Service") && Boolean(resource.namespace),
	);
	const detailsQueryKey = $derived(
		queryKeys.resourceDetails(resource, dynamicKindKey, kubeconfigSourceKey, yamlViewMode, yamlEncoding),
	);
	const yamlQueryKey = $derived(
		queryKeys.resourceYaml(resource, dynamicKindKey, kubeconfigSourceKey, yamlViewMode, yamlEncoding),
	);
	const eventsQueryKey = $derived(queryKeys.resourceEvents(resource, kubeconfigSourceKey));
	const detailsCancelScope = $derived(createCancelScope("resource-details", detailsQueryKey));
	const yamlCancelScope = $derived(createCancelScope("resource-yaml", yamlQueryKey));
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
		const currentYamlCancelScope = yamlCancelScope;
		const currentYamlQueryKey = yamlQueryKey;
		const currentEventsCancelScope = eventsCancelScope;
		const currentEventsQueryKey = eventsQueryKey;
		cancelPendingBackendScope(currentDetailsCancelScope);
		cancelPendingBackendScope(currentYamlCancelScope);
		cancelPendingBackendScope(currentEventsCancelScope);
		return () => {
			scheduleBackendScopeCancel(
				currentDetailsCancelScope,
				currentDetailsQueryKey,
				"detail.details.cancel",
			);
			scheduleBackendScopeCancel(
				currentYamlCancelScope,
				currentYamlQueryKey,
				"detail.yaml.cancel",
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
	const yamlQuery = createQuery<string>(() => ({
		queryKey: yamlQueryKey,
		queryFn: async () => {
			try {
				return await runDetailFetch("yaml", "resource-yaml", async () => {
					if (dynamicKind) {
						return (
							detailsQuery.data ??
							(await getDynamicResourceDetails(
								client,
								resource.cluster,
								dynamicKind,
								resource.name,
								resource.namespace ?? undefined,
								kubeconfigSourceKey,
								yamlViewMode,
								yamlEncoding,
								createCancellableRequest(yamlCancelScope, "yaml"),
							))
						).yaml;
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
					diagnosticLog("detail.yaml.cancel", { key: resourceKey });
				}
				throw error;
			}
		},
		enabled: yamlEnabled,
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
	const yamlText = $derived(yamlQuery.data ?? detailsQuery.data?.yaml ?? "");
	const yamlApplyDisabledReason = $derived(isYamlApplyDisabled(resource));
	const yamlApplyTarget = $derived(yamlApplyTargetLabel(resource));
	const canAllowYamlForceConflicts = $derived(
		!$settingsStore.allowYamlForceConflicts &&
			!yamlForceConflictsForResource &&
			isAppError(yamlPrepareRawError) &&
			yamlPrepareRawError.kind === "fieldManagerConflict",
	);
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
	const logRequest = $derived<PodLogStreamRequest | null>(
		isPod && selectedContainer
			? {
					clusterContext: resource.cluster,
					kubeconfigEnvVar: kubeconfigSourceKey,
					namespace: resource.namespace ?? "",
					podName: resource.name,
					container: selectedContainer,
					tailLines: 200,
				}
			: null,
	);
	const logSignature = $derived(logRequest ? JSON.stringify(logRequest) : "");
	const parsedLogLines = $derived(orderedLogLines(logLines, false));
	const incidentTimeline = $derived(
		buildIncidentTimeline({
			resource: detailResource,
			conditions: conditionRows,
			events: sortedEvents,
			containers: signalContainers,
			logLines: parsedLogLines,
		}),
	);
	const orderedVisibleLogLines = $derived(
		logLatestFirst ? [...parsedLogLines].reverse() : parsedLogLines,
	);
	const logFilterTerm = $derived(logFilter.trim().toLowerCase());
	const visibleLogLines = $derived(
		logFilterTerm
			? orderedVisibleLogLines.filter((line) =>
					line.raw.toLowerCase().includes(logFilterTerm),
				)
			: orderedVisibleLogLines,
	);

	$effect(() => {
		if (!isDetailTabAvailable(activeTab)) {
			activeTab = "details";
		}
	});

	$effect(() => {
		if (!isPod) {
			selectedContainer = "";
			return;
		}
		if (containerOptions.length === 0) return;
		if (!containerOptions.includes(selectedContainer)) {
			selectedContainer = containerOptions[0] ?? "";
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
		logSignature;
		logLines = [];
		logError = null;
	});

	$effect(() => {
		activeTab;
		logLines.length;
		visibleLogLines.length;
		logAutoFollow;
		logLatestFirst;
		logWrapLines;
		if (activeTab !== "logs" || !logAutoFollow || !logViewport) return;
		const frame = window.requestAnimationFrame(() => {
			if (!logViewport) return;
			logViewport.scrollTop = logLatestFirst ? 0 : logViewport.scrollHeight;
		});
		return () => window.cancelAnimationFrame(frame);
	});

	$effect(() => {
		resource.cluster;
		resource.kind;
		resource.name;
		resource.namespace;
		resetYamlApply();
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
					void queryClient.invalidateQueries({ queryKey: yamlQueryKey });
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

	$effect(() => {
		if (activeTab !== "logs" || !logRequest) {
			logStatus = "idle";
			logMessage = "Log stream idle";
			logError = null;
			return;
		}
		let cancelled = false;
		let streamId: string | null = null;
		logStatus = "connecting";
		logMessage = "Starting log stream";
		logError = null;
		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "started") {
				streamId = event.streamId;
				return;
			}
			if (event.type === "status") {
				logStatus = event.status;
				logMessage = event.message;
				return;
			}
			if (event.type === "logLine") {
				logStatus = "connected";
				logMessage = "Log stream connected";
				logLines = [...logLines, event.line].slice(-1_000);
				return;
			}
			if (event.type === "error") {
				logStatus = "error";
				logMessage = "Log stream error";
				logError = event.message;
				return;
			}
			if (event.type === "stopped") {
				logStatus = "stopped";
				logMessage = "Log stream stopped";
			}
		});
		void startPodLogStream(client, logRequest, channel)
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((error: unknown) => {
				if (cancelled) return;
				logStatus = "error";
				logMessage = "Log stream failed";
				logError = error;
			});
		return () => {
			cancelled = true;
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

	function setYamlViewMode(value: string) {
		yamlViewMode = value as YamlViewMode;
		resetYamlApply();
	}

	function setYamlEncoding(value: string) {
		yamlEncoding = value as YamlEncoding;
		resetYamlApply();
	}

	function isDetailTabAvailable(tab: DetailTab): boolean {
		if (tab === "logs") return canShowLogs;
		if (tab === "exec") return canShowExec;
		if (tab === "portForward") return canShowPortForward;
		return true;
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

<Tabs value={activeTab} onValueChange={tabChanged} class="min-h-0 gap-3">
	<TabsList variant="line" class="flex-wrap justify-start">
		<TabsTrigger value="details">Details</TabsTrigger>
		<TabsTrigger value="events">Events</TabsTrigger>
		{#if canShowLogs}<TabsTrigger value="logs">Logs</TabsTrigger>{/if}
		{#if canShowExec}<TabsTrigger value="exec">Exec</TabsTrigger>{/if}
		{#if canShowPortForward}<TabsTrigger value="portForward">Forward</TabsTrigger>{/if}
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

	<EventsTab
		{eventsQuery}
		{sortedEvents}
		{formatEventExactTime}
		{formatEventCompactTime}
	/>

	{#if canShowLogs}
		<LogsTab
			{isPod}
			{containerOptions}
			bind:selectedContainer
			{logStatus}
			{logMessage}
			{logError}
			bind:logFilter
			{logFilterTerm}
			bind:logWrapLines
			bind:logAutoFollow
			bind:logLatestFirst
			bind:logLines
			bind:logViewport
			{visibleLogLines}
			{parsedLogLines}
			{formatFullTimestamp}
			{formatLogTime}
		/>
	{/if}

	{#if canShowExec}<TabsContent value="exec">
		<ExecTab
			{client}
			{resource}
			containers={containerRows}
			bind:selectedContainer
			{kubeconfigSourceKey}
			active={activeTab === "exec"}
		/>
	</TabsContent>{/if}

	{#if canShowPortForward}<TabsContent value="portForward">
		<PortForwardTab
			{client}
			{resource}
			{kubeconfigSourceKey}
			yaml={yamlText}
			active={activeTab === "portForward"}
		/>
	</TabsContent>{/if}
</Tabs>

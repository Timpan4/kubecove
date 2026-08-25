<script lang="ts">
	import {
		closeStreamChannel,
		createStreamChannel,
		startAggregatedLogStream,
		startPodLogStream,
		stopStream,
		type TauriClient,
	} from "@/lib/tauri";
	import type {
		AggregatedLogStreamRequest,
		PodLogStreamRequest,
		ResourceSummary,
		StreamMessage,
	} from "@/lib/types";
	import LogsTab from "./LogsTab.svelte";
	import {
		appendParsedLogLine,
		logLineSearchText,
		type ParsedLogLine,
	} from "./log-helpers";

	type LogStreamRequest =
		| { mode: "pod"; request: PodLogStreamRequest }
		| { mode: "aggregate"; request: AggregatedLogStreamRequest }
		| null;

	let {
		client,
		resource,
		kubeconfigSourceKey,
		isPod,
		containerOptions,
		selectedContainer = $bindable(""),
		active,
		logFilter = $bindable(""),
		logWrapLines = $bindable(true),
		logAutoFollow = $bindable(true),
		logLatestFirst = $bindable(false),
		parsedLogLines = $bindable<ParsedLogLine[]>([]),
		formatFullTimestamp,
		formatLogTime,
	}: {
		client: TauriClient;
		resource: ResourceSummary;
		kubeconfigSourceKey?: string;
		isPod: boolean;
		containerOptions: string[];
		selectedContainer: string;
		active: boolean;
		logFilter: string;
		logWrapLines: boolean;
		logAutoFollow: boolean;
		logLatestFirst: boolean;
		parsedLogLines: ParsedLogLine[];
		formatFullTimestamp: (timestamp: string | null | undefined) => string;
		formatLogTime: (timestamp: string | undefined) => string;
	} = $props();

	let nextLogLineIndex = 0;
	let logStatus = $state("idle");
	let logMessage = $state("Log stream idle");
	let logError = $state<unknown>(null);
	let logViewport = $state<HTMLElement | null>(null);
	let logTailLines = $state(200);
	let logSinceSeconds = $state<number | undefined>(undefined);
	let logPaused = $state(false);

	const aggregateTargetKind = $derived(
		resource.kind === "Deployment" || resource.kind === "Service" ? resource.kind : null,
	);
	const isAggregateTarget = $derived(aggregateTargetKind !== null && Boolean(resource.namespace));
	const normalizedTailLines = $derived(
		Math.max(0, Math.floor(Number.isFinite(logTailLines) ? logTailLines : 0)),
	);
	const logRequest = $derived<LogStreamRequest>(
		isPod && selectedContainer
			? {
					mode: "pod",
					request: {
						clusterContext: resource.cluster,
						kubeconfigEnvVar: kubeconfigSourceKey,
						namespace: resource.namespace ?? "",
						podName: resource.name,
						container: selectedContainer,
						tailLines: normalizedTailLines,
						sinceSeconds: logSinceSeconds,
					},
				}
			: isAggregateTarget
				? {
						mode: "aggregate",
						request: {
							clusterContext: resource.cluster,
							kubeconfigEnvVar: kubeconfigSourceKey,
							namespace: resource.namespace ?? "",
							// SAFETY: isAggregateTarget requires aggregateTargetKind to be Deployment or Service.
							targetKind: aggregateTargetKind as "Deployment" | "Service",
							targetName: resource.name,
							tailLines: normalizedTailLines,
							sinceSeconds: logSinceSeconds,
						},
					}
				: null,
	);
	const logSignature = $derived(logRequest ? JSON.stringify(logRequest) : "");
	const orderedVisibleLogLines = $derived(
		logLatestFirst ? [...parsedLogLines].reverse() : parsedLogLines,
	);
	const logFilterTerm = $derived(logFilter.trim().toLowerCase());
	const visibleLogLines = $derived(
		logFilterTerm
			? orderedVisibleLogLines.filter((line) =>
					logLineSearchText(line).includes(logFilterTerm),
				)
			: orderedVisibleLogLines,
	);

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
		void logSignature;
		parsedLogLines = [];
		nextLogLineIndex = 0;
		logError = null;
	});

	$effect(() => {
		void active;
		void parsedLogLines.length;
		void visibleLogLines.length;
		void logAutoFollow;
		void logLatestFirst;
		void logWrapLines;
		if (!active || !logAutoFollow || !logViewport) return;
		const frame = window.requestAnimationFrame(() => {
			if (!logViewport) return;
			logViewport.scrollTop = logLatestFirst ? 0 : logViewport.scrollHeight;
		});
		return () => window.cancelAnimationFrame(frame);
	});

	function clearLogLines() {
		parsedLogLines = [];
		nextLogLineIndex = 0;
	}

	$effect(() => {
		if (!active || !logRequest || logPaused) {
			logStatus = logPaused ? "paused" : "idle";
			logMessage = logPaused ? "Log stream paused" : "Log stream idle";
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
				logMessage = logRequest.mode === "aggregate" ? "Aggregated logs connected" : "Log stream connected";
				appendParsedLogLine(
					parsedLogLines,
					{ line: event.line, source: event.source },
					nextLogLineIndex,
				);
				nextLogLineIndex += 1;
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
		const start =
			logRequest.mode === "aggregate"
				? startAggregatedLogStream(client, logRequest.request, channel)
				: startPodLogStream(client, logRequest.request, channel);
		void start
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((cause: unknown) => {
				if (cancelled) return;
				logStatus = "error";
				logMessage = "Log stream failed";
				logError = cause;
			});
		return () => {
			cancelled = true;
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});
</script>

<LogsTab
	{isPod}
	isAggregate={isAggregateTarget}
	targetKind={resource.kind}
	targetName={resource.name}
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
	lineCount={parsedLogLines.length}
	onClear={clearLogLines}
	bind:logViewport
	bind:logTailLines
	bind:logSinceSeconds
	bind:logPaused
	{visibleLogLines}
	{parsedLogLines}
	{formatFullTimestamp}
	{formatLogTime}
/>

<script lang="ts">
	import {
		closeStreamChannel,
		createStreamChannel,
		startAggregatedLogStream,
		startPodLogStream,
		stopStream,
		type TauriClient,
	} from "@/lib/tauri";
	import { settingsStore } from "@/lib/settings-store";
	import { onDestroy, untrack } from "svelte";
	import { SvelteSet } from "svelte/reactivity";
	import type {
		AggregatedLogStreamRequest,
		PodLogStreamRequest,
		ResourceSummary,
		LogLineSource,
		StreamMessage,
	} from "@/lib/types";
	import LogsTab from "./LogsTab.svelte";
	import {
		appendParsedLogBatch,
		filterOrderedParsedLogLines,
		MAX_RETAINED_LOG_LINES,
		orderParsedLogLines,
		type ParsedLogLine,
	} from "./log-helpers";
	import type { LogLevel } from "./log-structure";

	type LogStreamRequest =
		| { mode: "pod"; request: PodLogStreamRequest }
		| { mode: "aggregate"; request: AggregatedLogStreamRequest }
		| null;

	interface ScrollAnchor {
		row: HTMLElement;
		top: number;
	}

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
	let reconnectVersion = $state(0);
	let pendingLogLines: { line: string; source?: LogLineSource }[] = [];
	let incomingLogLines: { line: string; source?: LogLineSource }[] = [];
	let incomingCount = 0;
	let flushFrame: number | null = null;
	onDestroy(() => {
		if (flushFrame !== null) cancelAnimationFrame(flushFrame);
	});
	let pendingCount = $state(0);
	let appendedCount = $state(0);
	let seenAppendedCount = 0;
	let pinnedToLatest = $state(true);
	let programmaticScrollTop: number | null = null;
	let savedScrollTop = 0;
	let unseenCount = $state(0);
	const hiddenLevels = new SvelteSet<LogLevel>();
	const logLayout = $derived($settingsStore.logLayout);

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
		orderParsedLogLines(parsedLogLines, logLatestFirst),
	);
	const logFilterTerm = $derived(logFilter.trim().toLowerCase());
	const textMatchedLogLines = $derived(
		filterOrderedParsedLogLines(orderedVisibleLogLines, logFilterTerm),
	);
	// Unleveled lines stay visible: level chips only filter lines whose level was detected.
	const visibleLogLines = $derived(
		hiddenLevels.size === 0
			? textMatchedLogLines
			: textMatchedLogLines.filter((line) => {
					const level = line.structure.level;
					return !level || !hiddenLevels.has(level);
				}),
	);
	const levelCounts = $derived.by(() => {
		const counts = { error: 0, warn: 0, info: 0, debug: 0 } satisfies Record<LogLevel, number>;
		for (const line of parsedLogLines) {
			const level = line.structure.level;
			if (level) counts[level] += 1;
		}
		return counts;
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

	function resetLogLines() {
		if (flushFrame !== null) cancelAnimationFrame(flushFrame);
		flushFrame = null;
		incomingLogLines = [];
		incomingCount = 0;
		parsedLogLines = [];
		pendingLogLines = [];
		pendingCount = 0;
		appendedCount = 0;
		seenAppendedCount = 0;
		unseenCount = 0;
		pinnedToLatest = true;
	}

	$effect(() => {
		void logSignature;
		resetLogLines();
		logError = null;
	});

	function flushLogLines() {
		flushFrame = null;
		const entries = incomingLogLines;
		const count = incomingCount;
		incomingLogLines = [];
		incomingCount = 0;
		if (logPaused) {
			pendingLogLines = [...pendingLogLines, ...entries].slice(-MAX_RETAINED_LOG_LINES);
			pendingCount = pendingLogLines.length;
			return;
		}
		parsedLogLines = appendParsedLogBatch(parsedLogLines, entries, nextLogLineIndex + count - entries.length);
		nextLogLineIndex += count;
		appendedCount += count;
	}

	function queueLogLines(lines: string[], source?: LogLineSource) {
		for (const line of lines) incomingLogLines.push({ line, source });
		incomingCount += lines.length;
		if (incomingLogLines.length > MAX_RETAINED_LOG_LINES) {
			incomingLogLines = incomingLogLines.slice(-MAX_RETAINED_LOG_LINES);
		}
		flushFrame ??= requestAnimationFrame(flushLogLines);
	}

	function togglePause() {
		if (flushFrame !== null) {
			cancelAnimationFrame(flushFrame);
			flushLogLines();
		}
		if (!logPaused) {
			logPaused = true;
			return;
		}
		logPaused = false;
		const pending = pendingLogLines;
		pendingLogLines = [];
		pendingCount = 0;
		parsedLogLines = appendParsedLogBatch(parsedLogLines, pending, nextLogLineIndex);
		nextLogLineIndex += pending.length;
		appendedCount += pending.length;
	}

	function isAtLatest(viewport: HTMLElement): boolean {
		// MDN's scrolled-to-end check; the 1px tolerance absorbs fractional scroll offsets.
		return logLatestFirst
			? viewport.scrollTop <= 1
			: Math.abs(viewport.scrollHeight - viewport.clientHeight - viewport.scrollTop) <= 1;
	}

	function scrollToLatest(viewport: HTMLElement) {
		viewport.scrollTop = logLatestFirst ? 0 : viewport.scrollHeight;
		programmaticScrollTop = viewport.scrollTop;
	}

	function jumpToLatest() {
		pinnedToLatest = true;
		unseenCount = 0;
		if (logViewport) scrollToLatest(logViewport);
	}

	$effect.pre(() => {
		if (!active) savedScrollTop = untrack(() => logViewport?.scrollTop ?? savedScrollTop);
	});
	$effect(() => {
		if (!active) return;
		const viewport = untrack(() => logViewport);
		if (viewport && !untrack(() => pinnedToLatest)) {
			viewport.scrollTop = savedScrollTop;
			programmaticScrollTop = viewport.scrollTop;
		}
	});

	// Every visible row is a candidate so the anchor survives eviction of the top row.
	function findScrollAnchors(viewport: HTMLElement): ScrollAnchor[] {
		const anchors: ScrollAnchor[] = [];
		const bottom = viewport.scrollTop + viewport.clientHeight;
		for (const row of viewport.querySelectorAll<HTMLElement>("[data-log-row]")) {
			if (row.offsetTop >= bottom) break;
			if (row.offsetTop + row.offsetHeight > viewport.scrollTop) anchors.push({ row, top: row.offsetTop });
		}
		return anchors;
	}

	$effect(() => {
		const viewport = logViewport;
		if (!viewport) return;
		const onScroll = () => {
			if (!active) return;
			// Our own follow scroll reports late; lines appended meanwhile must not read as scrolling away.
			if (viewport.scrollTop === programmaticScrollTop) return;
			programmaticScrollTop = null;
			pinnedToLatest = isAtLatest(viewport);
			if (pinnedToLatest) unseenCount = 0;
		};
		viewport.addEventListener("scroll", onScroll, { passive: true });
		return () => viewport.removeEventListener("scroll", onScroll);
	});

	$effect(() => {
		void logLatestFirst;
		untrack(jumpToLatest);
	});

	$effect(() => {
		const count = appendedCount;
		const added = count - seenAppendedCount;
		seenAppendedCount = count;
		if (added > 0 && !untrack(() => pinnedToLatest)) {
			unseenCount = untrack(() => unseenCount) + added;
		}
	});

	// The viewport disables native overflow-anchor (WebKit lacks it), so keep the row the
	// user is reading in place when lines are prepended or evicted while scrolled away.
	let scrollAnchors: ScrollAnchor[] = [];
	$effect.pre(() => {
		void appendedCount;
		const viewport = untrack(() => logViewport);
		scrollAnchors =
			viewport && !untrack(() => pinnedToLatest) ? findScrollAnchors(viewport) : [];
	});

	$effect(() => {
		void appendedCount;
		const anchor = scrollAnchors.find((candidate) => candidate.row.isConnected);
		scrollAnchors = [];
		const viewport = untrack(() => logViewport);
		if (!anchor || !viewport) return;
		viewport.scrollTop += anchor.row.offsetTop - anchor.top;
	});

	$effect(() => {
		// visibleLogLines can be the same mutated array, so track the append counter and length.
		void appendedCount;
		void visibleLogLines.length;
		void logLayout;
		void logWrapLines;
		const viewport = logViewport;
		if (!active || !viewport || !pinnedToLatest) return;
		scrollToLatest(viewport);
	});

	$effect(() => {
		if (!active) return;
		untrack(() => {
			if (logStatus !== "stopped" && logStatus !== "error") return;
			// The stream request has no sinceTime cursor. Clear before re-tailing so
			// older servers and aggregate streams cannot replay retained lines.
			resetLogLines();
			reconnectVersion += 1;
		});
	});

	$effect(() => {
		// Keep healthy streams alive across tab switches; restart only on re-entry
		// after a stop/error or when the requested target/options change.
		void reconnectVersion;
		if (!logRequest) {
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
			if (event.type === "logLines" || event.type === "logLine") {
				logStatus = "connected";
				logMessage = logRequest.mode === "aggregate" ? "Aggregated logs connected" : "Log stream connected";
				queueLogLines(event.type === "logLine" ? [event.line] : event.lines, event.source);
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
		}, client);
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
	{active}
	{isPod}
	isAggregate={isAggregateTarget}
	{containerOptions}
	bind:selectedContainer
	{logStatus}
	{logMessage}
	{logError}
	bind:logFilter
	{logFilterTerm}
	bind:logWrapLines
	bind:logLatestFirst
	{logLayout}
	lineCount={parsedLogLines.length}
	onClear={resetLogLines}
	bind:logViewport
	bind:logTailLines
	bind:logSinceSeconds
	{logPaused}
	{pendingCount}
	onTogglePause={togglePause}
	{unseenCount}
	onJumpToLatest={jumpToLatest}
	{hiddenLevels}
	{levelCounts}
	{visibleLogLines}
	{formatFullTimestamp}
	{formatLogTime}
/>

// Opt-in recorder for interaction profiling, toggled from Settings -> Diagnostics.
// Each user input (click, drag, key, scroll) becomes one entry with input-to-paint
// time, frame pacing, and DOM mutation counts until the next input. Targets and
// mutation sources are Svelte source locations in dev builds, and test ids or
// roles in release builds; never text content or cluster data. Entries stay in
// memory; the dev server additionally mirrors them to e2e/artifacts.
import { getDiagnosticsSnapshot, isDiagnosticsEnabled, MAX_FRONTEND_EVENTS } from "./diagnostics";
import { describeTarget, resetSourceCache, sourceOf, subtreeSource } from "./interaction-sources";

type InteractionKind = "click" | "drag" | "key" | "scroll";

export interface RecordedInteraction {
	id: number;
	kind: InteractionKind;
	wallTime: string;
	target: string;
	durationMs: number;
	inputToPaint: { type: string; ms: number }[];
	frames: { count: number; refreshMs: number; p50Ms: number; p95Ms: number; maxMs: number; droppedFrames: number };
	dom: {
		added: number;
		removed: number;
		attributes: number;
		text: number;
		sources: { source: string; mutations: number }[];
	};
	commands?: { cmd: string; ms: number; ok: boolean; startOffsetMs: number }[];
}

interface Segment {
	id: number;
	kind: InteractionKind;
	startedAt: number;
	wallTime: string;
	target: string;
	inputToPaint: { type: string; ms: number }[];
	frameDeltas: number[];
	added: number;
	removed: number;
	attributes: number;
	text: number;
	bySource: Map<string, number>;
}

interface Session {
	abort: AbortController;
	observer: MutationObserver;
	frameRequest: number;
}

const DEV_SINK = process.env.KUBECOVE_PUBLIC_DEV_INTERACTIONS ?? "";
const recorded: RecordedInteraction[] = [];
// Session histogram of rAF deltas (0.1 ms buckets); its median is the refresh interval.
const frameHistogram = new Map<number, number>();
let histogramTotal = 0;
let session: Session | null = null;
let current: Segment | null = null;
let nextId = 1;
let pointerDown = false;

function refreshIntervalMs(): number {
	let seen = 0;
	for (const [bucket, count] of [...frameHistogram.entries()].sort((a, b) => a[0] - b[0])) {
		seen += count;
		if (seen * 2 >= histogramTotal) return bucket / 10;
	}
	return 0;
}

function percentile(sorted: number[], fraction: number): number {
	if (sorted.length === 0) return 0;
	return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))];
}

function round(value: number): number {
	return Math.round(value * 10) / 10;
}

function commandsDuring(start: number, end: number): RecordedInteraction["commands"] {
	if (!isDiagnosticsEnabled()) return undefined;
	const commands: NonNullable<RecordedInteraction["commands"]> = [];
	for (const event of getDiagnosticsSnapshot().frontendEvents) {
		if (event.event !== "tauri.invoke.done" && event.event !== "tauri.invoke.error") continue;
		const ms = Number(event.data.ms ?? 0);
		const commandStart = event.atMs - ms;
		if (commandStart < start || commandStart >= end) continue;
		commands.push({
			cmd: String(event.data.cmd),
			ms,
			ok: event.event === "tauri.invoke.done",
			startOffsetMs: Math.round(commandStart - start),
		});
	}
	return commands;
}

function serialize(segment: Segment, endedAt: number): RecordedInteraction {
	const refreshMs = refreshIntervalMs();
	const sorted = [...segment.frameDeltas].sort((a, b) => a - b);
	let droppedFrames = 0;
	if (refreshMs > 0) {
		for (const delta of segment.frameDeltas) {
			droppedFrames += Math.max(0, Math.round(delta / refreshMs) - 1);
		}
	}
	return {
		id: segment.id,
		kind: segment.kind,
		wallTime: segment.wallTime,
		target: segment.target,
		durationMs: Math.round(endedAt - segment.startedAt),
		inputToPaint: segment.inputToPaint,
		frames: {
			count: segment.frameDeltas.length,
			refreshMs: round(refreshMs),
			p50Ms: round(percentile(sorted, 0.5)),
			p95Ms: round(percentile(sorted, 0.95)),
			maxMs: round(sorted.at(-1) ?? 0),
			droppedFrames,
		},
		dom: {
			added: segment.added,
			removed: segment.removed,
			attributes: segment.attributes,
			text: segment.text,
			sources: [...segment.bySource.entries()]
				.sort((a, b) => b[1] - a[1])
				.map(([source, mutations]) => ({ source, mutations })),
		},
		commands: commandsDuring(segment.startedAt, endedAt),
	};
}

function flush(): void {
	if (!current) return;
	const interaction = serialize(current, performance.now());
	current = null;
	recorded.push(interaction);
	// Same retention as frontend diagnostic events.
	if (recorded.length > MAX_FRONTEND_EVENTS) recorded.splice(0, recorded.length - MAX_FRONTEND_EVENTS);
	if (!DEV_SINK) return;
	void fetch(DEV_SINK, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(interaction),
		keepalive: true,
	}).catch((error) => console.warn("[kubecove:interactions] dev sink post failed", error));
}

function begin(kind: InteractionKind, event: Event): void {
	flush();
	current = {
		id: nextId++,
		kind,
		startedAt: performance.now(),
		wallTime: new Date().toISOString(),
		target: describeTarget(event.target),
		inputToPaint: [],
		frameDeltas: [],
		added: 0,
		removed: 0,
		attributes: 0,
		text: 0,
		bySource: new Map(),
	};
}

// Time from dispatch of the input event to the frame that shows its result: a rAF
// callback runs before paint, and the message task after it runs once that frame
// is done. WebKitGTK's event.timeStamp is not on the performance.now() clock, so
// dispatch time is read in this capture listener; queueing before dispatch is excluded.
function measureInputToPaint(event: Event): void {
	const segment = current;
	if (!segment) return;
	const dispatchedAt = performance.now();
	requestAnimationFrame(() => {
		const channel = new MessageChannel();
		channel.port1.onmessage = () => {
			segment.inputToPaint.push({ type: event.type, ms: round(performance.now() - dispatchedAt) });
		};
		channel.port2.postMessage(null);
	});
}

function countMutation(segment: Segment, record: MutationRecord): void {
	let source: string;
	let weight = 1;
	switch (record.type) {
		case "childList":
			segment.added += record.addedNodes.length;
			segment.removed += record.removedNodes.length;
			source = subtreeSource(record);
			weight = record.addedNodes.length + record.removedNodes.length;
			break;
		case "attributes":
			segment.attributes += 1;
			source = sourceOf(record.target);
			break;
		default:
			segment.text += 1;
			source = sourceOf(record.target);
	}
	segment.bySource.set(source, (segment.bySource.get(source) ?? 0) + weight);
}

function trackFrame(previous: number) {
	return (now: number) => {
		if (!session) return;
		const delta = now - previous;
		if (document.visibilityState === "visible") {
			const bucket = Math.round(delta * 10);
			frameHistogram.set(bucket, (frameHistogram.get(bucket) ?? 0) + 1);
			histogramTotal += 1;
			current?.frameDeltas.push(delta);
		}
		session.frameRequest = requestAnimationFrame(trackFrame(now));
	};
}

export function isInteractionRecording(): boolean {
	return session !== null;
}

export function startInteractionRecording(): void {
	if (session) return;
	const abort = new AbortController();
	const options = { capture: true, passive: true, signal: abort.signal } as const;
	window.addEventListener("pointerdown", (event) => {
		pointerDown = true;
		begin("click", event);
		measureInputToPaint(event);
	}, options);
	window.addEventListener("pointermove", () => {
		if (pointerDown && current?.kind === "click") current.kind = "drag";
	}, options);
	window.addEventListener("pointerup", () => {
		pointerDown = false;
	}, options);
	window.addEventListener("click", measureInputToPaint, options);
	window.addEventListener("wheel", (event) => {
		if (current?.kind === "scroll") return;
		begin("scroll", event);
		measureInputToPaint(event);
	}, options);
	window.addEventListener("keydown", (event) => {
		if (event.repeat && current?.kind === "key") return;
		begin("key", event);
		measureInputToPaint(event);
	}, options);
	window.addEventListener("pagehide", flush, { signal: abort.signal });
	document.addEventListener("visibilitychange", () => {
		if (document.visibilityState === "hidden") flush();
	}, { signal: abort.signal });

	const observer = new MutationObserver((records) => {
		if (!current) return;
		for (const record of records) countMutation(current, record);
	});
	observer.observe(document.body, { subtree: true, childList: true, attributes: true, characterData: true });
	session = { abort, observer, frameRequest: requestAnimationFrame(trackFrame(performance.now())) };
}

export function stopInteractionRecording(): void {
	if (!session) return;
	flush();
	session.abort.abort();
	session.observer.disconnect();
	cancelAnimationFrame(session.frameRequest);
	session = null;
	pointerDown = false;
	resetSourceCache();
}

export function getRecordedInteractions(): RecordedInteraction[] {
	return [...recorded];
}

export function clearRecordedInteractions(): void {
	recorded.length = 0;
	current = null;
	frameHistogram.clear();
	histogramTotal = 0;
}

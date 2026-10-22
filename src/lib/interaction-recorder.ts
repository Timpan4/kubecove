// Opt-in recorder for interaction profiling, toggled from Settings -> Diagnostics.
// Each user input (click, drag, key, scroll) becomes one entry with input-to-paint
// time, frame pacing, and DOM mutation counts until the next input. Targets and
// mutation sources are Svelte source locations in dev builds, and test ids or
// roles in release builds; never text content or cluster data. Entries stay in
// memory; the dev server additionally mirrors them to e2e/artifacts.
import { getDiagnosticsSnapshot, isDiagnosticsEnabled, MAX_FRONTEND_EVENTS } from "./diagnostics";
import { addFrame, emptyHistogram, type FrameHistogram, frameDeltas, percentileMs } from "./interaction-frames";
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
	// Input-to-paint measurements still waiting for their frame; the dev sink post waits for them.
	pendingPaints: number;
	flushed: RecordedInteraction | null;
	frames: FrameHistogram;
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
// The session histogram's median is the refresh interval.
let sessionFrames = emptyHistogram();
let session: Session | null = null;
let current: Segment | null = null;
let nextId = 1;
let pointerDown = false;

function refreshIntervalMs(): number {
	return percentileMs(sessionFrames, 0.5);
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
	const deltas = frameDeltas(segment.frames);
	let droppedFrames = 0;
	if (refreshMs > 0) {
		for (const [deltaMs, count] of deltas) {
			droppedFrames += count * Math.max(0, Math.round(deltaMs / refreshMs) - 1);
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
			count: segment.frames.total,
			refreshMs: round(refreshMs),
			p50Ms: round(percentileMs(segment.frames, 0.5)),
			p95Ms: round(percentileMs(segment.frames, 0.95)),
			maxMs: round(deltas.at(-1)?.[0] ?? 0),
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

function postToDevSink(interaction: RecordedInteraction): void {
	if (!DEV_SINK) return;
	void fetch(DEV_SINK, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(interaction),
		keepalive: true,
	}).catch((error) => console.warn("[kubecove:interactions] dev sink post failed", error));
}

function flush(): void {
	if (!current) return;
	const segment = current;
	const interaction = serialize(segment, performance.now());
	current = null;
	segment.flushed = interaction;
	recorded.push(interaction);
	// Same retention as frontend diagnostic events.
	if (recorded.length > MAX_FRONTEND_EVENTS) recorded.splice(0, recorded.length - MAX_FRONTEND_EVENTS);
	if (segment.pendingPaints === 0) postToDevSink(interaction);
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
		pendingPaints: 0,
		flushed: null,
		frames: emptyHistogram(),
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
	segment.pendingPaints += 1;
	requestAnimationFrame(() => {
		const channel = new MessageChannel();
		channel.port1.onmessage = () => {
			segment.inputToPaint.push({ type: event.type, ms: round(performance.now() - dispatchedAt) });
			segment.pendingPaints -= 1;
			// A later input may have ended this segment before its frame painted.
			if (segment.flushed && segment.pendingPaints === 0) postToDevSink(segment.flushed);
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
			addFrame(sessionFrames, delta);
			if (current) addFrame(current.frames, delta);
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
	sessionFrames = emptyHistogram();
}

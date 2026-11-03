// Replays TLC's state graph of ResourceWatch.tla (Variant = "current") against the real
// observeResourceScope. Every edge is replayed from the initial state, one model action per
// real callback, and the code's observable state must match the model after each step.
import type { Channel } from "@tauri-apps/api/core";
import { readFileSync } from "node:fs";
import { observeResourceScope, type ResourceWatchState } from "../../../../src/lib/resource-watch";
import { createMockTauriClient } from "../../../../src/lib/tauri-runtime";
import type { StreamMessage } from "../../../../src/lib/types";

type ModelValue = string | number | boolean | ModelValue[];
type ModelState = {
	phase: string;
	// The pre-#635 registration flag; phase is "unused" in that variant.
	closed: boolean;
	connected: boolean;
	error: boolean;
	warning: boolean;
	call: string[];
	q: string[][];
	chOpen: boolean[];
	starts: number;
	stopCalls: number[];
};
type Edge = { from: string; to: string; action: string };
type CloseVia = "update" | "stop";
type Step = { action: string; n?: number; message?: string; closeVia: CloseVia };

// TLC values in DOT labels: strings, integers, booleans, <<sequences>>, and {sets}.
function parseValue(text: string): ModelValue {
	let at = 0;
	const skip = () => {
		while (text[at] === " " || text[at] === ",") at++;
	};
	const value = (): ModelValue => {
		skip();
		if (text.startsWith("<<", at) || text[at] === "{") {
			const close = text[at] === "{" ? "}" : ">>";
			at += close.length;
			const items: ModelValue[] = [];
			for (skip(); !text.startsWith(close, at); skip()) items.push(value());
			at += close.length;
			return items;
		}
		if (text[at] === '"') {
			const end = text.indexOf('"', at + 1);
			const literal = text.slice(at + 1, end);
			at = end + 1;
			return literal;
		}
		const token = /^(TRUE|FALSE|-?\d+)/.exec(text.slice(at))?.[0];
		if (!token) throw new Error(`cannot parse TLC value at ${JSON.stringify(text.slice(at, at + 20))}`);
		at += token.length;
		return token === "TRUE" ? true : token === "FALSE" ? false : Number(token);
	};
	return value();
}

function unescapeDot(label: string) {
	return label.replace(/\\(.)/g, (_, c: string) => (c === "n" ? "\n" : c));
}

export function readStateGraph(dotFile: string) {
	const states = new Map<string, ModelState>();
	const edges: Edge[] = [];
	let initial: string | undefined;
	for (const line of readFileSync(dotFile, "utf8").split("\n")) {
		const edge = /^(-?\d+) -> (-?\d+) \[label="(\w+)"/.exec(line);
		if (edge) {
			edges.push({ from: edge[1], to: edge[2], action: edge[3] });
			continue;
		}
		const node = /^(-?\d+) \[label="(.*)"(,style = filled)?\];?$/.exec(line);
		if (!node) continue;
		const fields: Record<string, ModelValue> = {};
		for (const conjunct of unescapeDot(node[2]).split("\n")) {
			const match = /^\/\\ (\w+) = (.*)$/.exec(conjunct);
			if (match) fields[match[1]] = parseValue(match[2]);
		}
		states.set(node[1], fields as unknown as ModelState);
		if (node[3]) initial = node[1];
	}
	if (!initial) throw new Error(`${dotFile} has no initial state`);
	return { states, edges, initial };
}

// The model action's parameter n is the start whose per-start variables the step changed.
function changedStart(from: ModelState, to: ModelState, changed: (n: number) => boolean) {
	for (let n = 1; n <= from.call.length; n++) if (changed(n)) return n;
	throw new Error(`no start changed in a ${JSON.stringify({ from, to })} step`);
}

function stepOf(edge: Edge, from: ModelState, to: ModelState, closeVia: CloseVia): Step {
	const i = (n: number) => n - 1;
	if (edge.action === "Deliver") {
		const n = changedStart(from, to, (k) => JSON.stringify(from.q[i(k)]) !== JSON.stringify(to.q[i(k)]));
		return { action: edge.action, n, message: from.q[i(n)][0], closeVia };
	}
	if (edge.action === "Resolve" || edge.action === "Reject") {
		const n = changedStart(from, to, (k) => from.call[i(k)] !== "done" && to.call[i(k)] === "done");
		return { action: edge.action, n, closeVia };
	}
	return { action: edge.action, closeVia };
}

const streamId = (n: number) => `stream-${n}`;

function message(n: number, kind: string): StreamMessage {
	if (kind === "error") return { type: "error", streamId: streamId(n), message: "watch failed" };
	if (kind === "stopped") return { type: "stopped", streamId: streamId(n) };
	if (kind === "connected" || kind === "reconnecting" || kind === "forbidden")
		return { type: "status", streamId: streamId(n), status: kind, message: `watch ${kind}` };
	throw new Error(`the model sent an unknown message ${kind}`);
}

const flush = () => new Promise<void>((done) => setImmediate(done));

// One real observer with a scripted backend. The recovery timer is captured, not scheduled.
async function startHarness() {
	const channels: Array<{ handler: (event: StreamMessage) => void; closed: boolean }> = [];
	const calls: Array<{ resolve: (id: string) => void; reject: (error: Error) => void }> = [];
	const stopCalls = new Set<number>();
	let published: ResourceWatchState | undefined;
	let timer: (() => void) | undefined;
	const realSetTimeout = globalThis.setTimeout;
	const realClearTimeout = globalThis.clearTimeout;
	// SAFETY: observeResourceScope only schedules its recovery tick; the replay fires it as RecoveryTick.
	globalThis.setTimeout = ((callback: () => void) => {
		timer = callback;
		return 0;
	}) as unknown as typeof setTimeout;
	globalThis.clearTimeout = (() => {}) as typeof clearTimeout;
	const handlers = { onState: (state: ResourceWatchState) => (published = state), onChange: () => {}, reload: async () => [] };
	const observer = observeResourceScope({
		...handlers,
		client: createMockTauriClient({
			start_resource_watch: () => new Promise<string>((resolve, reject) => calls.push({ resolve, reject })),
			stop_stream: (args: { streamId: string }) => {
				stopCalls.add(Number(args.streamId.replace("stream-", "")));
				return true;
			},
		}),
		clusterContext: "verify",
		keys: [{ resourceKind: { kind: "Pod" } }],
		// Same shape as createMockChannel, but the replay can see when the code closes it.
		openChannel: (handler) => {
			const channel = { closed: false, handler: handler as (event: StreamMessage) => void };
			channels.push(channel);
			const unregister = async () => {
				channel.closed = true;
			};
			// SAFETY: closeStreamChannel only calls unregister; the handler is called directly by Deliver.
			return { id: channels.length, onmessage: handler, unregister } as unknown as Channel<StreamMessage>;
		},
	});
	await flush();
	const restore = () => {
		globalThis.setTimeout = realSetTimeout;
		globalThis.clearTimeout = realClearTimeout;
	};
	async function run(step: Step) {
		const n = step.n ?? 0;
		if (step.action === "RecoveryTick" || step.action === "RecoveryTickSkips") {
			const tick = timer;
			if (!tick) throw new Error("the code scheduled no recovery tick");
			tick();
		} else if (step.action === "Close") {
			if (step.closeVia === "stop") observer.stop();
			else observer.update([], handlers);
		} else if (step.action === "Deliver") channels[n - 1].handler(message(n, step.message ?? ""));
		else if (step.action === "Resolve") calls[n - 1].resolve(streamId(n));
		else if (step.action === "Reject") calls[n - 1].reject(new Error("start refused"));
		// Accept, Refuse, Report, and EndStream are backend steps; the frontend sees them later.
		await flush();
	}
	function differences(model: ModelState) {
		const found: string[] = [];
		const expect = (what: string, real: unknown, wanted: unknown) => {
			if (JSON.stringify(real) !== JSON.stringify(wanted)) found.push(`${what}: code ${JSON.stringify(real)}, model ${JSON.stringify(wanted)}`);
		};
		expect("start_resource_watch calls", calls.length, model.starts);
		expect("open channels", channels.map((channel) => !channel.closed), model.chOpen.slice(0, model.starts));
		expect("stop_stream calls", [...stopCalls].sort(), [...model.stopCalls].sort());
		// After close the observer publishes for the remaining keys, so only open watches are compared.
		if (model.phase !== "closed" && !model.closed && published) {
			expect("error shown", published.error !== null, model.error);
			expect("permission warning shown", (published.warnings?.length ?? 0) > 0, model.warning);
			if (!model.warning) expect("live updates connected", published.status === "connected", model.connected);
		}
		return found;
	}
	return { run, differences, restore };
}

const describe = (step: Step) =>
	`${step.action}${step.n ? `(${step.n})` : ""}${step.message ? ` [${step.message}]` : ""}${step.action === "Close" ? ` via ${step.closeVia}()` : ""}`;

export async function replay(dotFile: string) {
	const { states, edges, initial } = readStateGraph(dotFile);
	const out = new Map<string, Edge[]>();
	for (const edge of edges) out.set(edge.from, [...(out.get(edge.from) ?? []), edge]);
	// Shortest path to each state, so each edge replays as path(from) + edge.
	const path = new Map<string, Edge[]>([[initial, []]]);
	for (const queue = [initial]; queue.length; ) {
		const from = queue.shift() as string;
		for (const edge of out.get(from) ?? []) {
			if (path.has(edge.to)) continue;
			path.set(edge.to, [...(path.get(from) ?? []), edge]);
			queue.push(edge.to);
		}
	}
	let replayed = 0;
	for (const edge of edges) {
		const prefix = path.get(edge.from);
		if (!prefix) continue;
		for (const closeVia of edge.action === "Close" || prefix.some((e) => e.action === "Close") ? (["update", "stop"] as const) : (["update"] as const)) {
			const harness = await startHarness();
			try {
				const steps: string[] = [];
				const initialDiff = harness.differences(states.get(initial) as ModelState);
				if (initialDiff.length) return { ok: false, replayed, steps, differences: initialDiff };
				for (const e of [...prefix, edge]) {
					const step = stepOf(e, states.get(e.from) as ModelState, states.get(e.to) as ModelState, closeVia);
					steps.push(describe(step));
					await harness.run(step);
					const differences = harness.differences(states.get(e.to) as ModelState);
					if (differences.length) return { ok: false, replayed, steps, differences };
				}
				replayed++;
			} finally {
				harness.restore();
			}
		}
	}
	return { ok: true, replayed, steps: [], differences: [], states: states.size, edges: edges.length };
}

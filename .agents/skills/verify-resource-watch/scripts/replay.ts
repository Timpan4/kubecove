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
		if (token === "TRUE") return true;
		if (token === "FALSE") return false;
		return Number(token);
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
	// TLC opens `strict digraph DiskGraph {` and `subgraph cluster_graph {` and closes each with a `}` line after the
	// last edge. A file cut anywhere leaves a block open, so the depth must return to 0.
	let depth = 0;
	let opened = false;
	for (const line of readFileSync(dotFile, "utf8").split("\n")) {
		if (/\{\s*$/.test(line)) {
			depth++;
			opened = true;
			continue;
		}
		if (line.trim() === "}") {
			depth--;
			continue;
		}
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
	if (!opened || depth !== 0) throw new Error(`${dotFile} ends before its graph is closed, so it is incomplete`);
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
	let stopped = false;
	// Fake timers: a tick fires only if it is still scheduled, firing consumes it, and clearTimeout cancels it.
	// So a tick() that stops rescheduling, or a stop() that leaves the timer running, shows up as a failure.
	const pending = new Map<number, () => void>();
	let nextHandle = 1;
	const realSetTimeout = globalThis.setTimeout;
	const realClearTimeout = globalThis.clearTimeout;
	// SAFETY: observeResourceScope only schedules its recovery tick; the replay fires it as RecoveryTick.
	globalThis.setTimeout = ((callback: () => void) => {
		const handle = nextHandle++;
		pending.set(handle, callback);
		return handle;
	}) as unknown as typeof setTimeout;
	globalThis.clearTimeout = ((handle: number) => {
		pending.delete(handle);
	}) as unknown as typeof clearTimeout;
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
			if (pending.size > 1) throw new Error(`the code has ${pending.size} recovery ticks scheduled at once`);
			const [scheduled] = [...pending];
			if (scheduled) {
				pending.delete(scheduled[0]);
				scheduled[1]();
			}
			// stop() cancels the timer, so a model step that changes nothing after stop() has no tick to fire.
			else if (!(step.action === "RecoveryTickSkips" && stopped)) throw new Error("the code has no recovery tick scheduled");
		} else if (step.action === "Close") {
			if (step.closeVia === "stop") {
				observer.stop();
				stopped = true;
				if (pending.size) throw new Error("stop() left the recovery timer scheduled");
			} else observer.update([], handlers);
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
		if (model.phase !== "closed" && !model.closed) {
			if (!published) found.push("published state: code none, model an open watch");
			else {
				expect("error shown", published.error !== null, model.error);
				expect("permission warning shown", (published.warnings?.length ?? 0) > 0, model.warning);
				if (!model.warning) expect("live updates connected", published.status === "connected", model.connected);
			}
		}
		return found;
	}
	return { run, differences, restore };
}

const describe = (step: Step) =>
	`${step.action}${step.n ? `(${step.n})` : ""}${step.message ? ` [${step.message}]` : ""}${step.action === "Close" ? ` via ${step.closeVia}()` : ""}`;

// `expectedStates` is TLC's distinct-state count. readStateGraph rejects a file cut short; the count also catches
// state lines the parser no longer recognizes.
export async function replay(dotFile: string, expectedStates: number) {
	let graph: ReturnType<typeof readStateGraph>;
	try {
		graph = readStateGraph(dotFile);
	} catch (error) {
		return { ok: false, graphInvalid: true, replayed: 0, steps: [], differences: [error instanceof Error ? error.message : String(error)] };
	}
	const { states, edges, initial } = graph;
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
	// A truncated graph, or a DOT format the parser no longer reads, must fail instead of replaying nothing.
	const problems: string[] = [];
	if (states.size !== expectedStates) problems.push(`${dotFile} has ${states.size} states, but TLC found ${expectedStates}`);
	if (edges.length === 0) problems.push(`${dotFile} has no transitions`);
	const dangling = edges.filter((edge) => !states.has(edge.from) || !states.has(edge.to)).length;
	if (dangling) problems.push(`${dangling} of ${edges.length} edges name a state the graph does not define`);
	const unreachable = [...states.keys()].filter((state) => !path.has(state)).length;
	if (unreachable) problems.push(`${unreachable} of ${states.size} states are unreachable from the initial state`);
	if (problems.length) return { ok: false, graphInvalid: true, replayed: 0, steps: [], differences: problems };
	let replayed = 0;
	for (const edge of edges) {
		// SAFETY: the checks above found every state reachable, and edge.from is a state.
		const prefix = path.get(edge.from) as Edge[];
		for (const closeVia of edge.action === "Close" || prefix.some((e) => e.action === "Close") ? (["update", "stop"] as const) : (["update"] as const)) {
			const harness = await startHarness();
			try {
				const steps: string[] = [];
				const initialDiff = harness.differences(states.get(initial) as ModelState);
				if (initialDiff.length) return { ok: false, replayed, steps, differences: initialDiff };
				for (const e of [...prefix, edge]) {
					const step = stepOf(e, states.get(e.from) as ModelState, states.get(e.to) as ModelState, closeVia);
					steps.push(describe(step));
					try {
						await harness.run(step);
					} catch (error) {
						return { ok: false, replayed, steps, differences: [error instanceof Error ? error.message : String(error)] };
					}
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

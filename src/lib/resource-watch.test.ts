import { afterEach, expect, jest, test } from "bun:test";
import { createMockChannel, createMockTauriClient } from "./tauri-runtime";
import {
	observeResourceScope,
	RESOURCE_RECOVERY_INTERVAL,
	type ResourceWatchState,
} from "./resource-watch";
import type { StreamMessage, WatchResourceKey } from "./types";

afterEach(() => jest.useRealTimers());

// Failure modes: forbidden-watch polling evicts permitted kinds or repeated
// failures delay recovery beyond the existing recovery interval.
test("permission recovery scopes reloads to denied kinds at the original interval", async () => {
	jest.useFakeTimers();
	const channels: Array<ReturnType<typeof createMockChannel<StreamMessage>>> = [];
	const requested: Array<WatchResourceKey[] | undefined> = [];
	const reloadTimes: number[] = [];
	let fail = true;
	const denied = { resourceKind: { kind: "Secret" }, namespace: "ns" };
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev", keys: [{ resourceKind: { kind: "Pod" }, namespace: "ns" }, denied],
		onState: () => {}, onChange: () => {},
		reload: async (keys) => {
			requested.push(keys);
			reloadTimes.push(Date.now());
			await Promise.resolve();
			jest.advanceTimersByTime(1);
			if (fail) throw new Error("forbidden");
		},
		openChannel: (handler) => { const channel = createMockChannel(handler); channels.push(channel); return channel; },
	});
	await Promise.resolve();
	try {
		channels[0]?.onmessage({ type: "status", streamId: "watch", status: "connected", message: "connected" });
		channels[1]?.onmessage({ type: "status", streamId: "watch", status: "forbidden", message: "denied" });
		jest.advanceTimersByTime(15_000);
		await Promise.resolve();
		await Promise.resolve();
		expect(requested).toEqual([[denied]]);
		jest.advanceTimersByTime(15_000);
		await Promise.resolve();
		await Promise.resolve();
		expect(requested).toHaveLength(2);
		fail = false;
		jest.advanceTimersByTime(15_000);
		await Promise.resolve();
		await Promise.resolve();
		expect(requested).toHaveLength(3);
		jest.advanceTimersByTime(15_000);
		await Promise.resolve();
		await Promise.resolve();
		expect(requested).toHaveLength(4);
		expect(reloadTimes.map((time) => time - (reloadTimes[0] ?? 0))).toEqual([0, 15_000, 30_000, 45_000]);
	} finally { stop(); }
});

// Failure modes: a denied kind delays a disconnected permitted watch or a
// network failure delays its next recovery attempt.
test.each(["disconnected", "reload failure"])("denied kinds do not delay %s recovery", async (reason) => {
	jest.useFakeTimers();
	const channels: Array<ReturnType<typeof createMockChannel<StreamMessage>>> = [];
	const requested: Array<WatchResourceKey[] | undefined> = [];
	let reloads = 0;
	let networkFailure = false;
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev",
		keys: [{ resourceKind: { kind: "Pod" } }, { resourceKind: { kind: "Secret" } }],
		onState: () => {}, onChange: () => {},
		reload: async (keys) => {
			requested.push(keys);
			reloads++;
			await Promise.resolve();
			jest.advanceTimersByTime(1);
			if (networkFailure) throw new Error("offline");
			return ["forbidden"];
		},
		openChannel: (handler) => { const channel = createMockChannel(handler); channels.push(channel); return channel; },
	});
	try {
		channels[0]?.onmessage({ type: "status", streamId: "watch", status: "connected", message: "connected" });
		channels[1]?.onmessage({ type: "status", streamId: "watch", status: "forbidden", message: "denied" });
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await Promise.resolve();
		await Promise.resolve();
		expect(reloads).toBe(1);
		if (reason === "disconnected") {
			channels[0]?.onmessage({ type: "error", streamId: "watch", message: "offline" });
		} else {
			networkFailure = true;
			jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
			await Promise.resolve();
			await Promise.resolve();
			expect(reloads).toBe(2);
		}
		const beforeRetry = reloads;
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await Promise.resolve();
		await Promise.resolve();
		expect(reloads).toBe(beforeRetry + 1);
		if (reason === "disconnected") expect(requested.at(-1)).toBeUndefined();
	} finally { stop(); }
});

// A list snapshot can predate reconnection. Catch up once after that list ends,
// even if multiple connected notifications arrive while it is in flight.
test("reconnection during a reload queues exactly one trailing catch-up", async () => {
	jest.useFakeTimers();
	let channel!: ReturnType<typeof createMockChannel<StreamMessage>>;
	let complete!: () => void;
	let reloads = 0;
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev", keys: [{ resourceKind: { kind: "Pod" } }],
		onState: () => {}, onChange: () => {},
		reload: () => ++reloads === 1 ? new Promise<void>((resolve) => { complete = resolve; }) : Promise.resolve(),
		openChannel: (handler) => { channel = createMockChannel(handler); return channel; },
	});
	try {
		channel.onmessage({ type: "error", streamId: "watch", message: "offline" });
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		channel.onmessage({ type: "status", streamId: "watch", status: "connected", message: "connected" });
		channel.onmessage({ type: "status", streamId: "watch", status: "connected", message: "renewed" });
		expect(reloads).toBe(1);
		complete();
		await Promise.resolve();
		expect(reloads).toBe(2);
		await Promise.resolve();
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		expect(reloads).toBe(2);
	} finally { stop(); }
});

test("a denied watch warns without interrupting permitted updates or retrying startup", async () => {
	jest.useFakeTimers();
	const channels: Array<ReturnType<typeof createMockChannel<StreamMessage>>> = [];
	const states: ResourceWatchState[] = [];
	let starts = 0;
	let changes = 0;
	let reloads = 0;
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => `watch-${++starts}`, stop_stream: () => true }),
		clusterContext: "dev",
		keys: [{ resourceKind: { kind: "Pod" }, namespace: "cert-manager" }, { resourceKind: { kind: "Secret" }, namespace: "cert-manager" }],
		onState: (state) => states.push(state),
		onChange: () => { changes++; },
		reload: async () => { reloads++; },
		openChannel: (handler) => { const channel = createMockChannel(handler); channels.push(channel); return channel; },
	});
	await Promise.resolve();
	try {
		channels[0]?.onmessage({ type: "status", streamId: "watch-1", status: "connected", message: "pods connected" });
		channels[1]?.onmessage({ type: "status", streamId: "watch-2", status: "forbidden", message: "Secret in namespace cert-manager: watch access denied." });
		expect(states.at(-1)).toMatchObject({ status: "partial", error: null, warnings: ["Secret in namespace cert-manager: watch access denied."] });
		channels[0]?.onmessage({ type: "resourceChanged", streamId: "watch-1", action: "modified", target: { cluster: "dev", kind: "Pod", name: "pod" } });
		expect(changes).toBe(1);
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await Promise.resolve();
		expect(reloads).toBe(1);
		expect(starts).toBe(2);
		channels[0]?.onmessage({ type: "error", streamId: "watch-1", message: "offline" });
		expect(states.at(-1)).toMatchObject({ status: "reconnecting", error: "offline" });
		channels[0]?.onmessage({ type: "status", streamId: "watch-1", status: "connected", message: "recovered" });
		expect(states.at(-1)?.status).toBe("partial");
	} finally { stop(); }
});

test("an entirely denied scope uses polling rather than reconnecting", async () => {
	let channel!: ReturnType<typeof createMockChannel<StreamMessage>>;
	const states: ResourceWatchState[] = [];
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev", keys: [{ resourceKind: { kind: "Pod" } }],
		onState: (state) => states.push(state), onChange: () => {}, reload: async () => {},
		openChannel: (handler) => { channel = createMockChannel(handler); return channel; },
	});
	await Promise.resolve();
	try {
		channel.onmessage({ type: "status", streamId: "watch", status: "forbidden", message: "Pod: watch access denied." });
		expect(states.at(-1)).toMatchObject({ status: "polling", error: null });
	} finally { stop(); }
});

test("healthy startup and watch renewals do not reload the view", async () => {
	jest.useFakeTimers();
	let channel!: ReturnType<typeof createMockChannel<StreamMessage>>;
	let reloads = 0;
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev",
		keys: [{ resourceKind: { kind: "Pod" } }],
		onState: () => {},
		onChange: () => {},
		reload: async () => { reloads++; },
		openChannel: (handler) => { channel = createMockChannel(handler); return channel; },
	});
	await Promise.resolve();
	try {
		channel.onmessage({ type: "status", streamId: "watch", status: "connected", message: "connected" });
		await Promise.resolve();
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		channel.onmessage({ type: "status", streamId: "watch", status: "connected", message: "renewed" });
		expect(reloads).toBe(0);
	} finally { stop(); }
});

test("a failed reload cannot mark a recovered watch as disconnected and clears after retry", async () => {
	jest.useFakeTimers();
	let channel!: ReturnType<typeof createMockChannel<StreamMessage>>;
	const states: ResourceWatchState[] = [];
	let rejectReload!: (error: Error) => void;
	let reloads = 0;
	const { stop } = observeResourceScope({
		client: createMockTauriClient({ start_resource_watch: () => "watch", stop_stream: () => true }),
		clusterContext: "dev",
		keys: [{ resourceKind: { kind: "Pod" } }],
		onState: (state) => states.push(state),
		onChange: () => {},
		reload: () => ++reloads === 1 ? new Promise<void>((_, reject) => { rejectReload = reject; }) : Promise.resolve(),
		openChannel: (handler) => { channel = createMockChannel(handler); return channel; },
	});
	await Promise.resolve();
	try {
		channel.onmessage({ type: "error", streamId: "watch", message: "offline" });
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		channel.onmessage({ type: "status", streamId: "watch", status: "connected", message: "recovered" });
		rejectReload(new Error("list unavailable"));
		await Promise.resolve();
		expect(states.at(-1)).toMatchObject({ status: "connected", error: null, reloadError: "list unavailable" });
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await Promise.resolve();
		expect(states.at(-1)).toMatchObject({ status: "connected", error: null, reloadError: null });
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		expect(reloads).toBe(2);
	} finally { stop(); }
});

test("a healthy kind cannot hide a failed watch; fallback reloads stop after recovery", async () => {
	jest.useFakeTimers();
	const channels: Array<ReturnType<typeof createMockChannel<StreamMessage>>> =
		[];
	const states: ResourceWatchState[] = [];
	let reloads = 0;
	let starts = 0;
	const stopped: unknown[] = [];
	const client = createMockTauriClient({
		start_resource_watch: () => `watch-${++starts}`,
		stop_stream: (args: { streamId: string }) => {
			stopped.push(args);
			return true;
		},
	});
	const { stop } = observeResourceScope({
		client,
		clusterContext: "dev",
		keys: [
			{ resourceKind: { kind: "Pod" } },
			{ resourceKind: { kind: "CustomResourceDefinition" } },
		],
		onState: (state) => states.push(state),
		onChange: () => {},
		reload: async () => {
			reloads++;
		},
		openChannel: (handler) => {
			const channel = createMockChannel(handler);
			channels.push(channel);
			return channel;
		},
	});
	await Promise.resolve();
	channels[0]?.onmessage({
		type: "status",
		streamId: "watch-1",
		status: "connected",
		message: "pods connected",
	});
	channels[1]?.onmessage({
		type: "error",
		streamId: "watch-2",
		message: "forbidden",
	});
	channels[0]?.onmessage({
		type: "resourceChanged",
		streamId: "watch-1",
		action: "modified",
		target: { cluster: "dev", kind: "Pod", name: "pod" },
	});
	expect(states.at(-1)).toMatchObject({
		status: "reconnecting",
		error: "forbidden",
	});
	jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
	await Promise.resolve();
	expect(reloads).toBe(1);
	channels[1]?.onmessage({
		type: "status",
		streamId: "watch-2",
		status: "connected",
		message: "connected",
	});
	await Promise.resolve();
	expect(states.at(-1)?.status).toBe("connected");
	expect(reloads).toBe(2);
	jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
	expect(reloads).toBe(2);
	stop();
	expect(stopped).toHaveLength(2);
});

test("startup rejection retries and a late start is stopped after disposal", async () => {
	jest.useFakeTimers();
	let starts = 0;
	let resolve!: (id: string) => void;
	const late = new Promise<string>((done) => {
		resolve = done;
	});
	const stopped: unknown[] = [];
	let failed!: () => void;
	const failure = new Promise<void>((done) => {
		failed = done;
	});
	let didStop!: () => void;
	const stoppedLate = new Promise<void>((done) => {
		didStop = done;
	});
	const { stop } = observeResourceScope({
		client: createMockTauriClient({
			start_resource_watch: () => {
				if (++starts === 1) throw new Error("offline");
				return late;
			},
			stop_stream: (args: { streamId: string }) => {
				stopped.push(args);
				didStop();
				return true;
			},
		}),
		clusterContext: "dev",
		keys: [{ resourceKind: { kind: "Pod" } }],
		onState: (state) => {
			if (state.error) failed();
		},
		onChange: () => {},
		reload: async () => {},
		openChannel: createMockChannel,
	});
	await failure;
	jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
	expect(starts).toBe(2);
	stop();
	resolve("late");
	await stoppedLate;
	expect(stopped).toEqual([{ streamId: "late" }]);
});

function reconcileHarness() {
	const started: string[] = [];
	const stopped: string[] = [];
	const handlers = new Map<string, (event: StreamMessage) => void>();
	const pending = new Map<string, (id: string) => void>();
	const keyId = (key: WatchResourceKey) => `${key.resourceKind.kind}/${key.namespace ?? "*"}`;
	const client = createMockTauriClient({
		start_resource_watch: (args: { keys: WatchResourceKey[] }) => {
			const id = keyId(args.keys[0]);
			started.push(id);
			if (id.startsWith("Slow/")) return new Promise<string>((done) => pending.set(id, done));
			return id;
		},
		stop_stream: (args: { streamId: string }) => {
			stopped.push(args.streamId);
			return true;
		},
	});
	let opened = 0;
	const openChannel = (handler: (event: StreamMessage) => void) => {
		// Keep the raw handler so tests can deliver events a closed channel would drop.
		handlers.set(String(opened++), handler);
		return createMockChannel(handler);
	};
	return { client, started, stopped, handlers, pending, openChannel };
}

const pod = (namespace: string) => ({ resourceKind: { kind: "Pod" }, namespace });

// Failure modes: a scope change restarts watches whose keys did not change,
// leaves removed watches running, or never starts added keys.
test("updating keys keeps unchanged watches and only starts or stops the difference", async () => {
	const harness = reconcileHarness();
	const observer = observeResourceScope({
		client: harness.client, clusterContext: "dev", keys: [pod("a"), pod("b")],
		onState: () => {}, onChange: () => {}, reload: async () => {},
		openChannel: harness.openChannel,
	});
	await Promise.resolve();
	await Promise.resolve();
	observer.update([pod("b"), pod("c")], { onState: () => {}, onChange: () => {}, reload: async () => {} });
	await Promise.resolve();
	await Promise.resolve();
	expect(harness.started).toEqual(["Pod/a", "Pod/b", "Pod/c"]);
	expect(harness.stopped).toEqual(["Pod/a"]);
	observer.stop();
	await Promise.resolve();
	expect(harness.stopped.sort()).toEqual(["Pod/a", "Pod/b", "Pod/c"]);
});

// Failure modes: a key removed while its start is in flight leaks the backend
// stream, or a removed watch still delivers changes into the new scope.
test("removed watches are stopped when their start resolves late and their events are ignored", async () => {
	const harness = reconcileHarness();
	const changes: string[] = [];
	const slow = { resourceKind: { kind: "Slow" }, namespace: "a" };
	const observer = observeResourceScope({
		client: harness.client, clusterContext: "dev", keys: [pod("a"), slow],
		onState: () => {}, onChange: (_event, key) => changes.push(`old:${key.namespace}`), reload: async () => {},
		openChannel: harness.openChannel,
	});
	await Promise.resolve();
	await Promise.resolve();
	observer.update([slow], {
		onState: () => {}, onChange: (_event, key) => changes.push(`new:${key.resourceKind.kind}`), reload: async () => {},
	});
	const removedPod = harness.handlers.get("0");
	const keptSlow = harness.handlers.get("1");
	const changed = (streamId: string): StreamMessage => ({
		type: "resourceChanged", streamId, action: "modified",
		target: { cluster: "dev", kind: "Pod", name: "x", namespace: "a" },
	});
	removedPod?.(changed("Pod/a"));
	keptSlow?.(changed("Slow/a"));
	expect(changes).toEqual(["new:Slow"]);

	observer.update([], { onState: () => {}, onChange: () => {}, reload: async () => {} });
	harness.pending.get("Slow/a")?.("Slow/a");
	await new Promise((done) => setTimeout(done, 0));
	expect(harness.stopped.sort()).toEqual(["Pod/a", "Slow/a"]);
	observer.stop();
});

// Failure mode: status keeps reporting a removed disconnected watch, so the
// view shows "reconnecting" after its scope no longer includes that key.
test("status after an update reflects only the current keys", async () => {
	const harness = reconcileHarness();
	const states: ResourceWatchState[] = [];
	const onState = (state: ResourceWatchState) => states.push(state);
	const observer = observeResourceScope({
		client: harness.client, clusterContext: "dev", keys: [pod("a"), pod("b")],
		onState, onChange: () => {}, reload: async () => {},
		openChannel: harness.openChannel,
	});
	await Promise.resolve();
	await Promise.resolve();
	harness.handlers.get("0")?.({ type: "status", streamId: "Pod/a", status: "connected", message: "connected" });
	expect(states.at(-1)?.status).toBe("reconnecting");
	observer.update([pod("a")], { onState, onChange: () => {}, reload: async () => {} });
	expect(states.at(-1)?.status).toBe("connected");
	observer.stop();
});

// Failure modes: reload warnings from a removed scope keep the view "partial" after the
// switch, or a reload still running for the old scope writes its warnings back afterward.
test("removing keys clears the old scope's reload warnings, including a late reload", async () => {
	jest.useFakeTimers();
	const harness = reconcileHarness();
	const states: ResourceWatchState[] = [];
	const onState = (state: ResourceWatchState) => states.push(state);
	let finishOldReload!: (warnings: string[]) => void;
	const secrets = { resourceKind: { kind: "Secret" }, namespace: "a" };
	const observer = observeResourceScope({
		client: harness.client, clusterContext: "dev", keys: [pod("a"), secrets], onState, onChange: () => {},
		reload: () => new Promise<string[]>((done) => (finishOldReload = done)),
		openChannel: harness.openChannel,
	});
	await Promise.resolve();
	await Promise.resolve();
	harness.handlers.get("0")?.({ type: "status", streamId: "Pod/a", status: "connected", message: "connected" });
	harness.handlers.get("1")?.({ type: "status", streamId: "Secret/a", status: "forbidden", message: "denied" });
	jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
	observer.update([pod("a")], { onState, onChange: () => {}, reload: async () => [] });
	finishOldReload(["Secrets cannot be listed"]);
	await Promise.resolve();
	await Promise.resolve();
	expect(states.at(-1)?.status).toBe("connected");
	expect(states.at(-1)?.warnings ?? []).toEqual([]);
	observer.stop();
});

// Failure mode: adding a namespace changes the queried scope too, so a reload still running
// for the old scope reports its error into the new one.
test("adding keys discards a reload that is still running for the old scope", async () => {
	jest.useFakeTimers();
	const harness = reconcileHarness();
	const states: ResourceWatchState[] = [];
	const onState = (state: ResourceWatchState) => states.push(state);
	let failOldReload!: (error: Error) => void;
	const observer = observeResourceScope({
		client: harness.client, clusterContext: "dev", keys: [pod("a")], onState, onChange: () => {},
		reload: () => new Promise<string[]>((_, fail) => (failOldReload = fail)),
		openChannel: harness.openChannel,
	});
	await Promise.resolve();
	await Promise.resolve();
	jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
	observer.update([pod("a"), pod("b")], { onState, onChange: () => {}, reload: async () => [] });
	failOldReload(new Error("old scope read failed"));
	await Promise.resolve();
	await Promise.resolve();
	expect(states.at(-1)?.reloadError ?? null).toBeNull();
	observer.stop();
});

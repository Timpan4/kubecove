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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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
	const stop = observeResourceScope({
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

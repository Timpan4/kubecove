import { afterEach, expect, jest, test } from "bun:test";
import { createMockChannel, createMockTauriClient } from "./tauri-runtime";
import {
	observeResourceScope,
	RESOURCE_RECOVERY_INTERVAL,
	type ResourceWatchState,
} from "./resource-watch";
import type { StreamMessage } from "./types";

afterEach(() => jest.useRealTimers());

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

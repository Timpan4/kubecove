import { afterEach, describe, expect, jest, test } from "bun:test";
import { QueryClient, QueryObserver, type QueryKey } from "@tanstack/svelte-query";
import {
	bindLiveView,
	createLiveReadRefresh,
	createWatchRefreshThrottle,
	type LiveView,
	type LiveViewState,
} from "./live-view-refresh";
import { configureQueryCatalog, queryKeys, queryMatchers } from "./queryKeys";
import { RESOURCE_RECOVERY_INTERVAL } from "./resource-watch";
import { createMockChannel, createMockTauriClient } from "./tauri-runtime";
import type { StreamMessage, WatchResourceKey } from "./types";

const cleanups: (() => void)[] = [];

afterEach(() => {
	for (const cleanup of cleanups.splice(0)) cleanup();
	jest.useRealTimers();
});

// Failure modes: trailing updates get lost, continuous churn postpones forever,
// overlapping requests accumulate, failures wedge the throttle, disposal refetches old scope.
describe("watch refresh throttle", () => {
	test("watch bursts refresh immediately and retain the last event at the throttle boundary", async () => {
		jest.useFakeTimers();
		let calls = 0;
		const throttle = createWatchRefreshThrottle(async () => {
			calls++;
		});
		throttle.notify();
		expect(calls).toBe(1);
		await Promise.resolve();
		throttle.notify();
		jest.advanceTimersByTime(249);
		expect(calls).toBe(1);
		jest.advanceTimersByTime(1);
		expect(calls).toBe(2);
		await Promise.resolve();
		jest.advanceTimersByTime(250);
		expect(calls).toBe(2);
		throttle.dispose();
	});

	test("events during an in-flight refresh produce one trailing refresh without overlap", async () => {
		jest.useFakeTimers();
		let calls = 0;
		let finish!: () => void;
		const throttle = createWatchRefreshThrottle(() => {
			calls++;
			return new Promise<void>((resolve) => {
				finish = resolve;
			});
		});
		throttle.notify();
		throttle.notify();
		jest.advanceTimersByTime(2000);
		throttle.notify();
		expect(calls).toBe(1);
		finish();
		await Promise.resolve();
		expect(calls).toBe(2);
		throttle.notify();
		throttle.dispose();
		finish();
		await Promise.resolve();
		jest.advanceTimersByTime(2000);
		expect(calls).toBe(2);
	});

	test("a rejected refresh allows subsequent events and disposal cancels a queued trailing call", async () => {
		jest.useFakeTimers();
		let calls = 0;
		const throttle = createWatchRefreshThrottle(async () => {
			calls++;
			throw new Error("offline");
		});
		throttle.notify();
		await Promise.resolve();
		throttle.notify();
		jest.advanceTimersByTime(250);
		expect(calls).toBe(2);
		await Promise.resolve();
		throttle.notify();
		throttle.dispose();
		jest.advanceTimersByTime(250);
		expect(calls).toBe(2);
	});
});

// Failure modes: a change refreshes reads another kind keeps fresh, or misses a
// read declared with an apiVersion when the watch names its group; a change during
// a running read is lost; a hidden read refetches; permission recovery rereads
// reads the denied kind does not feed; the recovery or refresh scope leaks past
// the declared reads; watches or pending refreshes outlive the view (ADR 0009).

const settle = () => new Promise((resolve) => setTimeout(resolve, 10));
const pods = { resourceKind: { kind: "Pod" }, namespace: "shop" };
const secrets = { resourceKind: { kind: "Secret" }, namespace: "shop" };
const podsKey = queryKeys.resources("dev", [{ kind: "Pod", namespace: "shop" }]);
const secretsKey = queryKeys.resources("dev", [{ kind: "Secret", namespace: "shop" }]);

function liveHarness() {
	const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	configureQueryCatalog(queryClient);
	const channels: ReturnType<typeof createMockChannel<StreamMessage>>[] = [];
	const stopped: string[] = [];
	const evicted: WatchResourceKey[][] = [];
	const states: LiveViewState[] = [];
	const fetches = new Map<string, number>();
	let streams = 0;
	const client = createMockTauriClient({
		start_resource_watch: () => `watch-${++streams}`,
		stop_stream: ({ streamId }: { streamId: string }) => {
			stopped.push(streamId);
			return true;
		},
		refresh_resource_cache: ({ keys }: { keys: WatchResourceKey[] }) => {
			evicted.push(keys);
			return { clearedEntries: 0 };
		},
	});
	cleanups.push(() => queryClient.clear());
	return {
		queryClient,
		channels,
		stopped,
		evicted,
		states,
		fetches: (name: string) => fetches.get(name) ?? 0,
		/** Observes a read. Uncached reads fetch on mount; `gate` holds each fetch until it resolves. */
		observe(name: string, queryKey: QueryKey, { cached = true, gate }: { cached?: boolean; gate?: () => Promise<void> } = {}) {
			if (cached) queryClient.setQueryData(queryKey, "cached");
			const stop = new QueryObserver(queryClient, {
				queryKey,
				queryFn: async () => {
					fetches.set(name, (fetches.get(name) ?? 0) + 1);
					await gate?.();
					return name;
				},
			}).subscribe(() => {});
			cleanups.push(stop);
		},
		bind(view: Partial<LiveView>) {
			const binding = bindLiveView({
				client,
				queryClient,
				view: { clusterContext: "dev", namespaces: ["shop"], keys: [], reads: [], refreshScope: "view", ...view },
				onState: (state) => states.push(state),
				openChannel: (handler) => {
					const channel = createMockChannel(handler);
					channels.push(channel);
					return channel;
				},
			});
			cleanups.push(binding.stop);
			return binding;
		},
		change(index: number, kind: string, streamId = "watch") {
			channels[index]?.onmessage({ type: "resourceChanged", streamId, action: "modified", target: { cluster: "dev", kind, name: "item", namespace: "shop" } });
		},
		status(index: number, status: string) {
			channels[index]?.onmessage({ type: "status", streamId: "watch", status, message: status });
		},
	};
}

// Failure modes: a final catch-up read fails with no later event, a timer keeps
// rereading after success, or disposal refreshes a resource the user has left.
describe("detail and event live read recovery", () => {
	test("a failed final catch-up retries without a later event and stops after success", async () => {
		jest.useFakeTimers();
		const live = liveHarness();
		let fail = true;
		live.observe("details", podsKey, { gate: async () => { if (fail) throw new Error("offline"); } });
		const refresh = createLiveReadRefresh(live.queryClient, queryMatchers.exact(podsKey));
		cleanups.push(refresh.dispose);
		refresh.notify();
		await live.queryClient.getQueryCache().find({ queryKey: podsKey })?.promise?.catch(() => {});
		fail = false;
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await live.queryClient.getQueryCache().find({ queryKey: podsKey })?.promise?.catch(() => {});
		expect(live.fetches("details")).toBe(2);
		expect(live.queryClient.getQueryData(podsKey)).toBe("details");
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		expect(live.fetches("details")).toBe(2);
	});
	test("disposing a failed read cancels its recovery", async () => {
		jest.useFakeTimers();
		const live = liveHarness();
		live.observe("events", podsKey, { gate: async () => { throw new Error("offline"); } });
		const refresh = createLiveReadRefresh(live.queryClient, queryMatchers.exact(podsKey));
		refresh.notify();
		await live.queryClient.getQueryCache().find({ queryKey: podsKey })?.promise?.catch(() => {});
		refresh.dispose();
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		expect(live.fetches("events")).toBe(1);
	});
});

describe("live view refresh binding", () => {
	// A connected watch alone does not prove its last list succeeded. A final
	// delete event must recover even when no later event arrives to trigger a read.
	test("a failed watch refresh exposes stale data and retries without another event", async () => {
		jest.useFakeTimers();
		const live = liveHarness();
		let fail = true;
		live.observe("pods", podsKey, { gate: async () => { if (fail) throw new Error("list unavailable"); } });
		live.bind({ keys: [pods], reads: [{ keys: [pods], match: queryMatchers.exact(podsKey) }] });
		live.status(0, "connected");
		live.change(0, "Pod");
		await live.queryClient.getQueryCache().find({ queryKey: podsKey })?.promise?.catch(() => {});
		expect(live.states.at(-1)?.reloadError).toContain("list unavailable");
		expect(live.queryClient.getQueryData(podsKey)).toBe("cached");
		fail = false;
		const recovered = new Promise<void>((resolve) => {
			const stop = live.queryClient.getQueryCache().subscribe(({ query }) => {
				if (query.state.data === "pods") { stop(); resolve(); }
			});
			cleanups.push(stop);
		});
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await recovered;
		expect(live.queryClient.getQueryData(podsKey)).toBe("pods");
		expect(live.states.at(-1)?.reloadError).toBeNull();
	});
	test("a change refreshes only the reads its watched kind keeps fresh", async () => {
		const live = liveHarness();
		live.observe("pods", podsKey);
		live.observe("secrets", secretsKey);
		live.bind({
			keys: [pods, secrets],
			reads: [
				{ keys: [pods], match: queryMatchers.exact(podsKey) },
				{ keys: [secrets], match: queryMatchers.exact(secretsKey) },
			],
		});
		live.change(0, "Pod");
		await settle();

		expect(live.fetches("pods")).toBe(1);
		expect(live.fetches("secrets")).toBe(0);
	});

	test("a read declared with an apiVersion is kept fresh by a watch that names the same group", async () => {
		const live = liveHarness();
		const applications = queryKeys.argoApps("dev");
		live.observe("applications", applications);
		const discovered = { resourceKind: { kind: "Application", group: "argoproj.io", apiVersion: "argoproj.io/v1alpha1" }, namespace: "argocd" };
		const otherGroup = { resourceKind: { kind: "Application", group: "other.io", apiVersion: "other.io/v1" }, namespace: "argocd" };
		const otherNamespace = { ...discovered, namespace: "shop" };
		live.bind({
			keys: [otherGroup, otherNamespace, discovered],
			reads: [{ keys: [{ resourceKind: { kind: "Application", apiVersion: "argoproj.io/v1alpha1" }, namespace: "argocd" }], match: queryMatchers.exact(applications) }],
		});
		live.change(0, "Application");
		live.change(1, "Application");
		await settle();
		expect(live.fetches("applications")).toBe(0);

		live.change(2, "Application");
		await settle();
		expect(live.fetches("applications")).toBe(1);
	});

	test("a change seen while its read runs refreshes that read again once it ends", async () => {
		const live = liveHarness();
		let release!: () => void;
		// The mount fetch may have listed before the change, so it cannot absorb it.
		live.observe("pods", podsKey, { cached: false, gate: () => new Promise<void>((resolve) => { release = resolve; }) });
		live.bind({ keys: [pods], reads: [{ keys: [pods], match: queryMatchers.exact(podsKey) }] });
		live.change(0, "Pod");
		await settle();
		expect(live.fetches("pods")).toBe(1);

		release();
		await new Promise((resolve) => setTimeout(resolve, 300));
		expect(live.fetches("pods")).toBe(2);
		release();
	});

	test("a read the view is not showing is marked stale without refetching", async () => {
		const live = liveHarness();
		const topology = queryKeys.resourceTopology("dev", ["shop"], "ownership");
		live.observe("topology", topology);
		live.bind({ keys: [pods], reads: [{ keys: [pods], match: queryMatchers.exact(topology), refetch: false }] });
		live.change(0, "Pod");
		await settle();

		expect(live.queryClient.getQueryState(topology)?.isInvalidated).toBe(true);
		expect(live.fetches("topology")).toBe(0);
	});

	test("a view without watched kinds is idle and opens no streams", () => {
		const live = liveHarness();
		live.bind({ keys: [] });

		expect(live.channels).toHaveLength(0);
		expect(live.states).toEqual([expect.objectContaining({ status: "idle" })]);
	});

	// Failure mode: rebinding on every view change restarts each stream on a namespace or kind switch.
	test("updating the watched keys keeps unchanged streams and starts or stops only the difference", async () => {
		const live = liveHarness();
		const deployments = { resourceKind: { kind: "Deployment" }, namespace: "shop" };
		const podReads = [{ keys: [pods], match: queryMatchers.exact(podsKey) }];
		live.observe("pods", podsKey);
		const binding = live.bind({ keys: [pods, secrets], reads: podReads });
		await settle();

		binding.update({ clusterContext: "dev", namespaces: ["shop"], keys: [pods, deployments], reads: podReads, refreshScope: "view" });
		await settle();

		expect(live.channels).toHaveLength(3);
		expect(live.stopped).toEqual(["watch-2"]);
		live.change(0, "Pod");
		await settle();
		expect(live.fetches("pods")).toBe(1);
	});

	test("permission recovery rereads only what the denied kind keeps fresh", async () => {
		jest.useFakeTimers();
		const flush = async () => {
			for (let step = 0; step < 20; step++) await Promise.resolve();
		};
		const live = liveHarness();
		live.observe("pods", podsKey);
		live.observe("secrets", secretsKey);
		live.bind({
			keys: [pods, secrets],
			reads: [
				{ keys: [pods], match: queryMatchers.exact(podsKey) },
				{ keys: [secrets], match: queryMatchers.exact(secretsKey) },
			],
		});
		await flush();
		live.status(0, "connected");
		live.status(1, "forbidden");
		jest.advanceTimersByTime(RESOURCE_RECOVERY_INTERVAL);
		await flush();

		expect(live.states.at(-1)?.status).toBe("partial");
		expect(live.evicted).toEqual([[secrets]]);
		expect(live.fetches("secrets")).toBe(1);
		expect(live.fetches("pods")).toBe(0);
	});

	test.each([
		["view", 1],
		["reads", 0],
	] as const)("manual refresh with scope %s rereads the declared reads and %i other view reads", async (refreshScope, otherReads) => {
		const live = liveHarness();
		live.observe("pods", podsKey);
		live.observe("namespaces", queryKeys.namespaces("dev"));
		const binding = live.bind({ keys: [pods], reads: [{ keys: [pods], match: queryMatchers.exact(podsKey) }], refreshScope });
		await binding.refresh();

		expect(live.evicted).toEqual([[pods]]);
		expect(live.fetches("pods")).toBe(1);
		expect(live.fetches("namespaces")).toBe(otherReads);
	});

	test("stopping the view stops its streams and ignores later changes", async () => {
		const live = liveHarness();
		live.observe("pods", podsKey);
		const binding = live.bind({ keys: [pods, secrets], reads: [{ keys: [pods], match: queryMatchers.exact(podsKey) }] });
		await settle();
		binding.stop();
		live.change(0, "Pod");
		await settle();

		expect(live.stopped.sort()).toEqual(["watch-1", "watch-2"]);
		expect(live.fetches("pods")).toBe(0);
	});
});

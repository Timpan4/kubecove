import { afterEach, describe, expect, test } from "bun:test";
import { QueryClient, QueryObserver, type QueryKey } from "@tanstack/svelte-query";
import { finiteRead, installFiniteReadLifecycle } from "./finite-read-lifecycle";
import { configureQueryCatalog, queryKeys } from "./queryKeys";
import type { CancellableRequest } from "./types";

// Failure modes: the backend scope is cancelled before the frontend query
// (ADR 0009 order), a remount or key change in the same task loses its read, an
// observer of another query protects or exposes the wrong read, a remount during
// frontend cancellation has its new backend request cancelled, the backend is
// asked to cancel a scope the read never used, reads without a backend request
// cause backend calls, and a cancelled read loses its `cancelled` kind.

const settle = () => new Promise((resolve) => setTimeout(resolve, 10));
const cleanups: (() => void)[] = [];

afterEach(() => {
	for (const cleanup of cleanups.splice(0)) cleanup();
});

function lifecycle() {
	const queryClient = new QueryClient();
	configureQueryCatalog(queryClient);
	const events: string[] = [];
	const backendScopes: string[] = [];
	const uninstall = installFiniteReadLifecycle(queryClient, async (scope) => {
		events.push("backend");
		backendScopes.push(scope);
		return { cancelled: 1 };
	});
	// TanStack reverts a cancelled fetch to idle with a setState action.
	const unwatch = queryClient.getQueryCache().subscribe((event) => {
		if (event.type === "updated" && event.action.type === "setState" && event.query.state.fetchStatus === "idle") {
			events.push("frontend");
		}
	});
	cleanups.push(() => {
		unwatch();
		uninstall();
		queryClient.clear();
	});
	return { queryClient, events, backendScopes };
}

function pendingRead(queryKey: QueryKey) {
	const requests: CancellableRequest[] = [];
	const results: ((value: string) => void)[] = [];
	const options = finiteRead({
		queryKey,
		label: "test.read",
		read: (request) => {
			requests.push(request);
			return new Promise<string>((resolve) => results.push(resolve));
		},
	});
	return { options, requests, resolve: (value: string) => results.at(-1)?.(value) };
}

function observe(queryClient: QueryClient, options: ReturnType<typeof pendingRead>["options"]) {
	return new QueryObserver(queryClient, options).subscribe(() => {});
}

const topologyKey = queryKeys.resourceTopology("dev", ["default"], "ownership");
const metricsKey = queryKeys.resourceMetrics("dev", ["default"]);

describe("finite read lifecycle", () => {
	test("an unobserved read is cancelled in the next task, frontend before backend", async () => {
		const { queryClient, events, backendScopes } = lifecycle();
		const read = pendingRead(topologyKey);
		const unsubscribe = observe(queryClient, read.options);
		expect(read.requests).toHaveLength(1);

		unsubscribe();
		expect(events).toEqual([]);
		await settle();

		expect(events).toEqual(["frontend", "backend"]);
		expect(backendScopes).toEqual([read.requests[0].cancelScope]);
	});

	test("a remount in the same task keeps the running read and its result", async () => {
		const { queryClient, events } = lifecycle();
		const read = pendingRead(topologyKey);
		observe(queryClient, read.options)();
		cleanups.push(observe(queryClient, read.options));
		await settle();
		read.resolve("fresh");
		await settle();

		expect(events).toEqual([]);
		expect(read.requests).toHaveLength(1);
		expect(queryClient.getQueryData(topologyKey)).toBe("fresh");
	});

	test("a key change cancels the previous key's read and starts the next", async () => {
		const { queryClient, backendScopes } = lifecycle();
		const previous = pendingRead(topologyKey);
		const next = pendingRead(metricsKey);
		const observer = new QueryObserver(queryClient, previous.options);
		cleanups.push(observer.subscribe(() => {}));

		observer.setOptions(next.options);
		await settle();

		expect(backendScopes).toEqual([previous.requests[0].cancelScope]);
		expect(next.requests).toHaveLength(1);
		expect(next.requests[0].cancelScope).not.toBe(previous.requests[0].cancelScope);
	});

	test("only an observer of the exact query protects its read", async () => {
		const { queryClient, backendScopes } = lifecycle();
		const shared = pendingRead(topologyKey);
		const alone = pendingRead(metricsKey);
		const firstShared = observe(queryClient, shared.options);
		cleanups.push(observe(queryClient, shared.options));
		const onlyAlone = observe(queryClient, alone.options);

		firstShared();
		onlyAlone();
		await settle();

		expect(backendScopes).toEqual([alone.requests[0].cancelScope]);
	});

	test("a remount during frontend cancellation keeps its new backend request", async () => {
		const { queryClient, events } = lifecycle();
		const read = pendingRead(topologyKey);
		const remount = queryClient.getQueryCache().subscribe((event) => {
			if (event.type === "updated" && event.action.type === "setState") {
				remount();
				cleanups.push(observe(queryClient, read.options));
			}
		});
		observe(queryClient, read.options)();
		await settle();

		expect(events).toEqual(["frontend"]);
		expect(read.requests).toHaveLength(2);
		expect(read.requests[1].cancelScope).toBe(read.requests[0].cancelScope);
		expect(read.requests[1].requestId).not.toBe(read.requests[0].requestId);
	});

	test("finite reads without a backend request finish into the cache", async () => {
		const { queryClient, events } = lifecycle();
		const namespacesKey = queryKeys.namespaces("dev");
		let finish!: (value: string) => void;
		const unsubscribe = new QueryObserver(queryClient, {
			queryKey: namespacesKey,
			queryFn: () => new Promise<string>((resolve) => { finish = resolve; }),
		}).subscribe(() => {});

		unsubscribe();
		await settle();
		finish("namespaces");
		await settle();

		expect(events).toEqual([]);
		expect(queryClient.getQueryData(namespacesKey)).toBe("namespaces");
	});

	test("a read cancelled by the backend keeps the cancelled kind", async () => {
		const { queryClient } = lifecycle();
		const read = finiteRead({
			queryKey: topologyKey,
			label: "test.read",
			read: async () => {
				throw { kind: "cancelled", message: "Request was cancelled" };
			},
		});

		await expect(queryClient.fetchQuery({ ...read, retry: false })).rejects.toMatchObject({
			kind: "cancelled",
		});
	});
});

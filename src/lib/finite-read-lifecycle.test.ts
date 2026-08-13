import { describe, expect, test } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import {
	configureFiniteReadQueryDefaults,
	createCancelScope,
	createFiniteReadCleanup,
	createFiniteReadRequest,
	finiteReadMeta,
	isFiniteReadQuery,
} from "./finite-read-lifecycle";

function settleCancellation(): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, 10));
}

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function observedQuery(queryClient: QueryClient, queryKey: readonly unknown[]) {
	queryClient.setQueryData(queryKey, "cached");
	const observer = new QueryObserver(queryClient, { queryKey, enabled: false });
	return observer.subscribe(() => {});
}

describe("finite read lifecycle", () => {
	test("classifies finite reads through TanStack metadata", async () => {
		const queryClient = new QueryClient();
		configureFiniteReadQueryDefaults(queryClient);
		await queryClient.fetchQuery({
			queryKey: ["resources", "default"],
			queryFn: async () => [],
			meta: finiteReadMeta({ namespace: "default" }),
		});
		queryClient.setQueryData(["argo-server-discovery", "default", "kind-dev"], []);
		queryClient.setQueryData(["port-forwards"], []);

		const finite = queryClient.getQueryCache().find({
			queryKey: ["resources", "default"],
			exact: true,
		});
		const discovery = queryClient.getQueryCache().find({
			queryKey: ["argo-server-discovery", "default", "kind-dev"],
			exact: true,
		});
		const live = queryClient.getQueryCache().find({
			queryKey: ["port-forwards"],
			exact: true,
		});

		expect(finite && isFiniteReadQuery(finite)).toBe(true);
		expect(discovery && isFiniteReadQuery(discovery)).toBe(true);
		expect(finite?.options.meta?.namespace).toBe("default");
		expect(live && isFiniteReadQuery(live)).toBe(false);
	});

	test("derives a stable scope and unique backend request identity", () => {
		const queryKey = ["resource-details", "default", "Pod", "api"] as const;
		const scope = createCancelScope("resource-details", queryKey);
		const first = createFiniteReadRequest(scope, "details");
		const second = createFiniteReadRequest(scope, "details");

		expect(scope).toBe(
			'resource-details:["resource-details","default","Pod","api"]',
		);
		expect(first.cancelScope).toBe(scope);
		expect(second.cancelScope).toBe(scope);
		expect(first.requestId).not.toBe(second.requestId);
	});

	test("cancels frontend before backend when exact query is unobserved", async () => {
		const queryClient = new QueryClient();
		const queryKey = ["resources", "stale"] as const;
		queryClient.setQueryData(queryKey, []);
		const events: string[] = [];
		queryClient.cancelQueries = async () => {
			events.push("frontend");
		};
		const cleanup = createFiniteReadCleanup(queryClient, async () => {
			events.push("backend");
			return { cancelled: 1 };
		});

		cleanup.schedule("resources:stale", queryKey);
		await settleCancellation();

		expect(events).toEqual(["frontend", "backend"]);
	});

	test("next-task exact observer protects only its query", async () => {
		const queryClient = new QueryClient();
		queryClient.cancelQueries = async () => {};
		const observedKey = ["resources", "current"] as const;
		const staleKey = ["resources", "stale"] as const;
		const unsubscribe = observedQuery(queryClient, observedKey);
		queryClient.setQueryData(staleKey, []);
		const cancelled: string[] = [];
		const cleanup = createFiniteReadCleanup(queryClient, async (scope) => {
			cancelled.push(scope);
			return { cancelled: 1 };
		});

		cleanup.schedule("resources:current", observedKey);
		cleanup.schedule("resources:stale", staleKey);
		await settleCancellation();

		expect(cancelled).toEqual(["resources:stale"]);
		unsubscribe();
	});

	test("remount during frontend cancellation protects the new backend request", async () => {
		const queryClient = new QueryClient();
		const queryKey = ["resource-details", "current"] as const;
		queryClient.setQueryData(queryKey, {});
		const frontendCancellation = deferred();
		queryClient.cancelQueries = () => frontendCancellation.promise;
		let backendCalls = 0;
		const cleanup = createFiniteReadCleanup(queryClient, async () => {
			backendCalls += 1;
			return { cancelled: 1 };
		});

		cleanup.schedule("resource-details:current", queryKey);
		await settleCancellation();
		const unsubscribe = observedQuery(queryClient, queryKey);
		frontendCancellation.resolve();
		await settleCancellation();

		expect(backendCalls).toBe(0);
		unsubscribe();
	});

	test("retaining a scope cancels its pending cleanup", async () => {
		const queryClient = new QueryClient();
		const queryKey = ["resource-details", "current"] as const;
		queryClient.setQueryData(queryKey, {});
		let backendCalls = 0;
		const cleanup = createFiniteReadCleanup(queryClient, async () => {
			backendCalls += 1;
			return { cancelled: 1 };
		});

		cleanup.schedule("resource-details:current", queryKey);
		cleanup.cancelPending("resource-details:current");
		await settleCancellation();

		expect(backendCalls).toBe(0);
	});
});

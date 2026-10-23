import { expect, test } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { configureQueryCatalog, queryKeys } from "./queryKeys";
import { refreshCurrentView } from "./resource-refresh";
import { createMockTauriClient } from "./tauri-runtime";
import { listNamespaces, refreshResourceCache } from "./tauri";
import type { ResourceSummary } from "./types";

const pod: ResourceSummary = { kind: "Pod", cluster: "dev", name: "pod", namespace: "default", age: "1d", health: "healthy" };

// Failure modes: automatic catch-up skips inspector/discovery/metrics reads,
// cancels a running read, or permission recovery refreshes unrelated families.
test.each([false, true])("automatic recovery waits for running reads, permission-only: %s", async (permissionOnly) => {
	const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	configureQueryCatalog(queryClient);
	const resourceKey = queryKeys.resources("dev", []);
	const keys = [
		resourceKey,
		queryKeys.resourceDetails(pod, undefined, { redactSecrets: true }),
		queryKeys.resourceYaml(pod, undefined, { redactSecrets: true }),
		queryKeys.resourceEvents(pod),
		queryKeys.namespaces("dev"),
		queryKeys.resourceMetrics("dev", []),
	];
	const calls: string[] = [];
	let finish!: () => void;
	let aborted = false;
	const stops = keys.map((key) => {
		queryClient.setQueryData(key, "cached");
		return new QueryObserver(queryClient, {
			queryKey: key,
			queryFn: async ({ signal }) => {
				calls.push(String(key[0]));
				if (key === keys[0] && calls.length === 1) {
					signal.addEventListener("abort", () => { aborted = true; });
					await new Promise<void>((resolve) => { finish = resolve; });
				}
				return "fresh";
			},
		}).subscribe(() => {});
	});
	try {
		const running = queryClient.refetchQueries({ queryKey: keys[0], exact: true });
		const recovery = refreshCurrentView({
			queryClient,
			client: createMockTauriClient({ refresh_resource_cache: () => { calls.push("evict"); return { clearedEntries: 1 }; } }),
			clusterContext: "dev", keys: [], namespaces: [],
			reads: permissionOnly ? [resourceKey] : undefined,
			cancelRefetch: false,
		});
		await Promise.resolve();
		expect(calls).toEqual(["resources"]);
		expect(aborted).toBe(false);
		finish();
		await running;
		await recovery;
		expect(aborted).toBe(false);
		expect(calls).toEqual(["resources", "evict", ...(permissionOnly ? ["resources"] : keys.map((key) => String(key[0])))]);
	} finally { for (const stop of stops) stop(); queryClient.clear(); }
});

test("cache clearing never rejoins a discovery read started before refresh", async () => {
	let finishOld!: (value: object[]) => void;
	const oldResult = new Promise<object[]>((done) => {
		finishOld = done;
	});
	let calls = 0;
	const client = createMockTauriClient({
		list_namespaces: () => (++calls === 1 ? oldResult : [{ name: "fresh", age: "1d" }]),
		refresh_resource_cache: { clearedEntries: 1 },
	});
	const old = listNamespaces(client, "dev");
	await refreshResourceCache(client, "dev", [], []);
	expect(await listNamespaces(client, "dev")).toEqual([{ name: "fresh", age: "1d" }]);
	finishOld([{ name: "old" }]);
	await old;
	expect(calls).toBe(2);
});

test("refresh evicts backend data before reloading only active reads in the selected source/context", async () => {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
	});
	configureQueryCatalog(queryClient);
	const calls: string[] = [];
	const keys = [
		queryKeys.resources("dev", []),
		queryKeys.argoApps("dev"),
		queryKeys.resources("prod", []),
		queryKeys.resources("dev", [], "OTHER"),
	] as const;
	const stops = keys.map((key) => {
		queryClient.setQueryData(key, "cached");
		return new QueryObserver(queryClient, {
			queryKey: key,
			queryFn: async () => {
				calls.push(String(key));
				return "fresh";
			},
		}).subscribe(() => {});
	});
	const inactive = queryKeys.argoAppSets("dev");
	queryClient.setQueryData(inactive, "inactive");
	try {
		await refreshCurrentView({
			queryClient,
			client: createMockTauriClient({
				refresh_resource_cache: () => {
					calls.push("evict");
					return { clearedEntries: 1 };
				},
			}),
			clusterContext: "dev",
			keys: [],
			namespaces: [],
		});
		expect(calls).toEqual(["evict", String(keys[0]), String(keys[1])]);
		expect(queryClient.getQueryData<string>(keys[0])).toBe("fresh");
		expect(queryClient.getQueryData<string>(keys[2])).toBe("cached");
		expect(queryClient.getQueryData<string>(inactive)).toBe("inactive");
	} finally {
		for (const stop of stops) stop();
		queryClient.clear();
	}
});

test("refresh reports read errors instead of treating invalidation as successful", async () => {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false, staleTime: Infinity } },
	});
	configureQueryCatalog(queryClient);
	const key = queryKeys.argoApps("dev");
	queryClient.setQueryData(key, []);
	const stop = new QueryObserver(queryClient, {
		queryKey: key,
		queryFn: async () => {
			throw new Error("offline");
		},
	}).subscribe(() => {});
	try {
		await expect(
			refreshCurrentView({
				queryClient,
				client: createMockTauriClient({
					refresh_resource_cache: { clearedEntries: 0 },
				}),
				clusterContext: "dev",
				keys: [],
				namespaces: [],
			}),
		).rejects.toThrow("offline");
		expect(queryClient.getQueryData<unknown[]>(key)).toEqual([]);
	} finally {
		stop();
		queryClient.clear();
	}
});

test("refresh keeps permitted reads fresh and returns denied reads as warnings", async () => {
	const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	configureQueryCatalog(queryClient);
	const allowed = queryKeys.resources("dev", []);
	const denied = queryKeys.argoApps("dev");
	queryClient.setQueryData(allowed, "old");
	queryClient.setQueryData(denied, []);
	const stops = [
		new QueryObserver(queryClient, { queryKey: allowed, queryFn: async () => "fresh" }).subscribe(() => {}),
		new QueryObserver(queryClient, { queryKey: denied, queryFn: async () => { throw { kind: "forbidden", message: "applications: access denied" }; } }).subscribe(() => {}),
	];
	try {
		const warnings = await refreshCurrentView({ queryClient, client: createMockTauriClient({ refresh_resource_cache: { clearedEntries: 0 } }), clusterContext: "dev", keys: [], namespaces: [] });
		expect(warnings).toEqual(["applications: access denied"]);
		expect(queryClient.getQueryData<string>(allowed)).toBe("fresh");
		expect(queryClient.getQueryState(denied)?.error).toMatchObject({ kind: "forbidden" });
	} finally { for (const stop of stops) stop(); queryClient.clear(); }
});

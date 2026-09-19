import { expect, test } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { configureFiniteReadQueryDefaults } from "./finite-read-lifecycle";
import { queryKeys } from "./queryKeys";
import { refreshCurrentView } from "./resource-refresh";
import { createMockTauriClient } from "./tauri-runtime";
import { listNamespaces, refreshResourceCache } from "./tauri";

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
	configureFiniteReadQueryDefaults(queryClient);
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
	configureFiniteReadQueryDefaults(queryClient);
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

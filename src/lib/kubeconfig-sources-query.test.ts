import { describe, expect, test } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { kubeconfigSourcesQueryOptions } from "./kubeconfig-sources-query";
import { getSettingsSnapshot } from "./settings-store";
import { partializeSettings, useSettingsState } from "./settings";
import { createMockTauriClient } from "./tauri";
import type { KubeconfigSourcesSummary } from "./types";

const sources: KubeconfigSourcesSummary = {
	kubeconfigEnvVar: "KUBECONFIG",
	paths: [{ path: "synthetic-config" }],
	sourceKey: "kubeconfigSource=test",
	sourceLabel: "Test sources",
	showSourceLabels: true,
	warnings: [],
};

describe("kubeconfig source query", () => {
	for (const consumerFirst of [false, true]) {
		test(`bootstrap and consumer join one request, consumer first: ${consumerFirst}`, async () => {
			const queryClient = new QueryClient();
			const originalSettings = useSettingsState.getState();
			const pending = Promise.withResolvers<KubeconfigSourcesSummary>();
			let calls = 0;
			const responses = { get_kubeconfig_sources: () => { calls += 1; return pending.promise; } };
			const bootstrapOptions = kubeconfigSourcesQueryOptions(createMockTauriClient(responses));
			const consumerOptions = kubeconfigSourcesQueryOptions(createMockTauriClient(responses));
			const observer = new QueryObserver(queryClient, consumerOptions);
			let unsubscribe = () => {};
			try {
				if (consumerFirst) unsubscribe = observer.subscribe(() => {});
				const bootstrap = queryClient.fetchQuery(bootstrapOptions).then((result) => {
					getSettingsSnapshot().setKubeconfigSources(result);
					return result;
				});
				if (!consumerFirst) unsubscribe = observer.subscribe(() => {});
				expect(calls).toBe(1);
				expect(observer.getCurrentResult().isPending).toBe(true);
				pending.resolve(sources);
				expect(await bootstrap).toEqual(sources);
				expect(observer.getCurrentResult().data).toEqual(sources);
				expect(getSettingsSnapshot().kubeconfigSourceKey).toBe(sources.sourceKey);
				expect(getSettingsSnapshot().kubeconfigSourceLabel).toBe(sources.sourceLabel);
				expect(JSON.stringify(partializeSettings(getSettingsSnapshot()))).not.toContain(sources.sourceKey);
				expect(JSON.stringify(getSettingsSnapshot())).not.toContain("synthetic-config");
				expect(getSettingsSnapshot()).not.toHaveProperty("client");
				await queryClient.fetchQuery(consumerOptions);
				expect(calls).toBe(1);
			} finally {
				unsubscribe();
				queryClient.clear();
				useSettingsState.setState(originalSettings, true);
			}
		});
	}

	test("shares deterministic failures and allows an explicit retry", async () => {
		const queryClient = new QueryClient();
		const error = { kind: "kubeconfig", message: "Invalid kubeconfig" };
		let calls = 0;
		const client = createMockTauriClient({ get_kubeconfig_sources: () => {
			calls += 1;
			if (calls === 1) throw error;
			return sources;
		} });
		const options = kubeconfigSourcesQueryOptions(client);
		const observer = new QueryObserver(queryClient, options);
		const unsubscribe = observer.subscribe(() => {});
		try {
			await expect(queryClient.fetchQuery(options)).rejects.toEqual(error);
			expect(calls).toBe(1);
			expect(observer.getCurrentResult().isError).toBe(true);
			expect(observer.getCurrentResult().error).toEqual(error);
			expect((await observer.refetch()).data).toEqual(sources);
			expect(calls).toBe(2);
		} finally {
			unsubscribe();
			queryClient.clear();
		}
	});
});

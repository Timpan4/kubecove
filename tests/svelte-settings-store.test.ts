import { afterEach, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { get } from "svelte/store";
import { useSettingsState } from "../src/lib/settings";
import {
	getSettingsSnapshot,
	settingsStore,
} from "../src/lib/settings-store";

afterEach(() => {
	useSettingsState.getState().setShowUsageFooter(false);
	useSettingsState.getState().setShowUnavailableGitOpsProviders(false);
	useSettingsState.getState().setShowFullTopologyOnSelection(false);
	useSettingsState.getState().setGitOpsViewMode("cards");
	useSettingsState.getState().setHelmViewMode("cards");
});

describe("svelte settings store", () => {
	test("mirrors shared settings through a Svelte readable store", () => {
		expect(get(settingsStore).showUsageFooter).toBe(false);

		useSettingsState.getState().setShowUsageFooter(true);

		expect(get(settingsStore).showUsageFooter).toBe(true);
		expect(getSettingsSnapshot().showUsageFooter).toBe(true);
	});

	test("exposes the full-topology selection setting to Svelte surfaces", () => {
		expect(get(settingsStore).showFullTopologyOnSelection).toBe(false);

		useSettingsState.getState().setShowFullTopologyOnSelection(true);

		expect(getSettingsSnapshot().showFullTopologyOnSelection).toBe(true);
	});

	test("exposes the persisted GitOps view mode to Svelte surfaces", () => {
		expect(get(settingsStore).gitOpsViewMode).toBe("cards");

		useSettingsState.getState().setGitOpsViewMode("list");

		expect(getSettingsSnapshot().gitOpsViewMode).toBe("list");
	});

	test("exposes the persisted Helm view mode to Svelte surfaces", () => {
		expect(get(settingsStore).helmViewMode).toBe("cards");

		useSettingsState.getState().setHelmViewMode("list");

		expect(getSettingsSnapshot().helmViewMode).toBe("list");
	});

	test("updates Svelte subscribers when shared settings change", () => {
		const values: boolean[] = [];
		const unsubscribe = settingsStore.subscribe((settings) => {
			values.push(settings.showUnavailableGitOpsProviders);
		});

		useSettingsState.getState().setShowUnavailableGitOpsProviders(true);
		unsubscribe();

		expect(values).toEqual([false, true]);
	});

	test("Svelte kubeconfig settings read source state through svelte-query", () => {
		const source = readFileSync(
			"src/app/svelte/KubeconfigSettings.svelte",
			"utf8",
		);

		expect(source).toContain('import { createQuery, useQueryClient } from "@tanstack/svelte-query";');
		expect(source).toContain("createQuery(() => kubeconfigSourcesQueryOptions(client))");
		expect(source).toContain("queryClient.setQueryData(KUBECONFIG_SOURCES_QUERY_KEY, next)");
		expect(source).not.toContain("onMount");
	});

	test("settings surfaces warn that full topology selection can affect performance", () => {
		const svelteSource = readFileSync("src/app/svelte/SettingsSurface.svelte", "utf8");

		expect(svelteSource).toContain("Keep full map visible during selection");
		expect(svelteSource).toContain("Large namespaces may render slower");
	});

	test("opens documentation through an exact Tauri URL permission", () => {
		const settingsSource = readFileSync("src/app/svelte/SettingsSurface.svelte", "utf8");
		const tauriSource = readFileSync("src-tauri/src/lib.rs", "utf8");
		const capability = JSON.parse(
			readFileSync("src-tauri/capabilities/default.json", "utf8"),
		);

		expect(settingsSource).toContain('import { openUrl } from "@tauri-apps/plugin-opener"');
		expect(settingsSource).toContain(
			'const WIKI_URL = "https://github.com/Timpan4/kubecove/wiki"',
		);
		expect(settingsSource).toContain("if (!isTauriRuntime()) return");
		expect(settingsSource).toContain("await openUrl(WIKI_URL)");
		expect(settingsSource).toContain("documentationError = error");
		expect(settingsSource).toContain(
			'fallbackTitle: "KubeCove could not open the documentation"',
		);
		expect(settingsSource).toMatch(
			/<a\s+[\s\S]*?href=\{WIKI_URL\}[\s\S]*?onclick=\{openDocumentation\}[\s\S]*?>/,
		);
		expect(settingsSource).toContain('target="_blank"');
		expect(settingsSource).toContain('rel="noreferrer"');
		expect(tauriSource).toContain(".plugin(tauri_plugin_opener::init())");
		expect(capability.permissions).toContainEqual({
			identifier: "opener:allow-open-url",
			allow: [{ url: "https://github.com/Timpan4/kubecove/wiki" }],
		});
	});

	test("owns Argo connection management and receives workspace context", () => {
		const settingsSource = readFileSync("src/app/svelte/SettingsSurface.svelte", "utf8");
		const surfacesSource = readFileSync("src/app/svelte/AppSurfaces.svelte", "utf8");
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const connectionSource = readFileSync(
			"src/app/svelte/ArgoConnectionSettings.svelte",
			"utf8",
		);

		expect(settingsSource).toContain('import ArgoConnectionSettings from "./ArgoConnectionSettings.svelte"');
		expect(settingsSource).toContain('<ArgoConnectionSettings {clusterContext} {workspaceId} {kubeconfigEnvVar} />');
		expect(surfacesSource).toContain("clusterContext={workspace.scope.clusterContext}");
		expect(surfacesSource).toContain("workspaceId={workspace.id}");
		expect(surfacesSource).toContain("kubeconfigEnvVar={workspaceReadContext.kubeconfigSourceKey}");
		expect(appSource).toContain("<SettingsSurface onBack={openWorkspaceLauncher} />");
		expect(connectionSource).toContain("eligibleArgoProfiles(");
		expect(connectionSource).toContain('kubeconfigEnvVar ?? ""');
		expect(connectionSource).toContain("Open Settings from a workspace to discover or connect an Argo CD server.");
		expect(connectionSource.match(/<Field orientation="horizontal">/g)).toHaveLength(2);
	});
});

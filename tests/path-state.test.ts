import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import type { ResourceSummary } from "../src/lib/types";
import {
	PATH_STATE_SESSION_KEY,
	decodePathStateSnapshot,
	defaultPathStateSnapshot,
	parsePathStateHash,
	pathForPathState,
	readPathState,
	resourceRefFromSummary,
	resourceSummaryFromRef,
	sanitizePathStateSnapshot,
	writePathState,
	type PathStateSnapshot,
	type PathStateWorkspaceSnapshot,
} from "../src/lib/path-state";

function makeStorage(): Storage {
	const values = new Map<string, string>();
	return {
		get length() {
			return values.size;
		},
		clear: () => values.clear(),
		getItem: (key) => values.get(key) ?? null,
		key: (index) => Array.from(values.keys())[index] ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	} as Storage;
}

const originalWindow = globalThis.window;

function installWindow(hash = "") {
	const fakeWindow = {
		sessionStorage: makeStorage(),
		location: { hash },
		history: {
			replaceState: (_data: unknown, _title: string, url?: string | URL | null) => {
				fakeWindow.location.hash = String(url ?? "");
			},
		},
	};
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: fakeWindow,
	});
	return fakeWindow;
}

beforeEach(() => {
	installWindow();
});

afterEach(() => {
	Object.defineProperty(globalThis, "window", {
		configurable: true,
		value: originalWindow,
	});
});

function workspaceSnapshot(
	overrides: Partial<PathStateWorkspaceSnapshot> = {},
): PathStateWorkspaceSnapshot {
	return {
		workspaceId: "workspace-1",
		viewMode: "resources",
		selectedNode: { type: "kind", section: "workloads", namespace: "prod", kind: "Pod" },
		expandedSections: ["section::workloads"],
		resourceInitialSearch: "",
		resourceInitialGitOpsFilter: "",
		resourceInitialHealthFilter: "all",
		resourceNamespaceOverride: null,
		focusedResource: null,
		restoreTargetResource: null,
		targetHelmRelease: null,
		targetGitOpsApplication: null,
		resources: null,
		detail: null,
		surfaces: null,
		...overrides,
	};
}

function snapshot(
	overrides: Partial<PathStateSnapshot> = {},
): PathStateSnapshot {
	return {
		version: 1,
		runtime: "svelte",
		launcherView: "workspaces",
		workspace: workspaceSnapshot(),
		...overrides,
	};
}

describe("path state", () => {
	test("writes readable hash and reads session snapshot", () => {
		const state = snapshot({
			workspace: workspaceSnapshot({
				resources: {
					selectedNamespaces: ["prod"],
					selectedKinds: ["Pod"],
					search: "api",
					gitOpsFilter: "argo:Application::checkout",
					healthFilter: "degraded",
					sortColumn: "age",
					sortDesc: true,
					pageIndex: 2,
					scopeEditorOpen: true,
					collapsedGroups: ["gitops:checkout"],
					topologyMode: "networkFlow",
					selectedTopologyNodeId: "pod:api",
					mapPanelOpen: true,
					tablePanelOpen: false,
				},
			}),
		});

		writePathState(state);

		expect(window.location.hash).toBe("#/workspace/workspace-1/resources");
		expect(window.sessionStorage.getItem(PATH_STATE_SESSION_KEY)).toContain("workspace-1");
		expect(readPathState()).toEqual(state);
	});

	test("parses coarse hash routes", () => {
		expect(pathForPathState(defaultPathStateSnapshot("settings"))).toBe("#/settings");
		expect(parsePathStateHash("#/workspaces")).toEqual(defaultPathStateSnapshot("workspaces"));
		expect(parsePathStateHash("#/workspace/dev%20box/helm")?.workspace).toMatchObject({
			workspaceId: "dev box",
			viewMode: "helm",
		});
	});

	test("falls back on invalid input", () => {
		expect(decodePathStateSnapshot("{")).toBeNull();
		expect(sanitizePathStateSnapshot({ version: 1, runtime: "legacy" })).toBeNull();

		installWindow("#/settings").sessionStorage.setItem(PATH_STATE_SESSION_KEY, "{");
		expect(readPathState()).toEqual(defaultPathStateSnapshot("settings"));
	});

	test("round-trips resource identity only", () => {
		const resource: ResourceSummary = {
			cluster: "kind-dev",
			kind: "Deployment",
			name: "api",
			namespace: "prod",
			age: "4d",
			health: "healthy",
			apiVersion: "apps/v1",
			group: "apps",
			version: "v1",
			plural: "deployments",
			namespaced: true,
			dynamic: true,
			status: "Available",
			ready: "3/3",
		};

		const ref = resourceRefFromSummary(resource);
		expect(ref).toEqual({
			cluster: "kind-dev",
			kind: "Deployment",
			name: "api",
			namespace: "prod",
			apiVersion: "apps/v1",
			group: "apps",
			version: "v1",
			plural: "deployments",
			namespaced: true,
			dynamic: true,
		});
		expect(resourceSummaryFromRef(ref)).toMatchObject({
			cluster: "kind-dev",
			kind: "Deployment",
			name: "api",
			namespace: "prod",
			health: "unknown",
		});
	});

	test("strips unsafe and fetched payload fields", () => {
		const safe = sanitizePathStateSnapshot({
			version: 1,
			runtime: "svelte",
			launcherView: "workspaces",
			commandPaletteOpen: true,
			workspace: {
				...workspaceSnapshot(),
				focusedResource: {
					cluster: "kind-dev",
					kind: "Pod",
					name: "api-0",
					namespace: "prod",
					yaml: "secret",
					status: "Running",
				},
				detail: {
					activeTab: "yaml",
					logLines: ["secret log"],
					yamlDraft: "secret draft",
					yamlViewMode: "applyClean",
					yamlEncoding: "kyaml",
				},
				resources: {
					selectedNamespaces: ["prod"],
					selectedKinds: ["Pod"],
					error: "boom",
					fetchedRows: [{ name: "api-0" }],
				},
			},
		});

		expect(safe?.workspace?.focusedResource).toEqual({
			cluster: "kind-dev",
			kind: "Pod",
			name: "api-0",
			namespace: "prod",
		});
		expect(safe?.workspace?.detail).toMatchObject({
			activeTab: "yaml",
			yamlViewMode: "applyClean",
			yamlEncoding: "kyaml",
		});
		expect((safe?.workspace?.detail as Record<string, unknown>).yamlDraft).toBeUndefined();
		expect((safe?.workspace?.detail as Record<string, unknown>).logLines).toBeUndefined();
		expect((safe?.workspace?.resources as Record<string, unknown>).fetchedRows).toBeUndefined();
	});
});

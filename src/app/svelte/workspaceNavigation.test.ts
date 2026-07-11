import { createWorkspaceRecord } from "@/lib/workspace-model";
import type { ArgoApplicationSummary } from "@/lib/gitops-types";
import type { PathStateResourceBrowserState } from "@/lib/path-state";
import type { ResourceSummary } from "@/lib/types";
import {
	createWorkspaceNavigation,
	navigateWorkspace,
	workspaceNavigationSnapshot,
} from "./workspaceNavigation";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toMatchObject(expected: unknown): void;
};

const workspace = createWorkspaceRecord({
	name: "Ops",
	clusterContext: "kind-dev",
	namespaces: ["payments"],
});

const resource: ResourceSummary = {
	cluster: "kind-dev",
	kind: "Deployment",
	name: "checkout",
	namespace: "payments",
	age: "1d",
	health: "healthy",
};

const argoApplication = {
	cluster: "kind-dev",
	name: "checkout",
	age: "1d",
	namespace: "argocd",
} as ArgoApplicationSummary;

describe("workspace navigation", () => {
	test("restores only a matching workspace path", () => {
		const matching = createWorkspaceNavigation(workspace, {
			workspaceId: workspace.id,
			viewMode: "argo",
			selectedNode: null,
			expandedSections: ["section:argo"],
			resourceInitialSearch: "checkout",
			resourceInitialGitOpsFilter: "",
			resourceInitialHealthFilter: "all",
			resourceNamespaceOverride: null,
			focusedResource: null,
			restoreTargetResource: null,
			targetHelmRelease: null,
			targetGitOpsApplication: "checkout",
			resources: null,
			detail: null,
			surfaces: null,
		});
		const different = createWorkspaceNavigation(workspace, {
			...workspaceNavigationSnapshot(matching, {
				workspaceId: "different-workspace",
				expandedSections: [],
				detail: null,
				surfaces: null,
			}),
		});

		expect(matching.viewMode).toBe("argo");
		expect(matching.selectedNode).toBe(null);
		expect(matching.targetGitOpsApplication).toBe("checkout");
		expect(different.viewMode).toBe("overview");
		expect(different.selectedNode).toEqual({
			type: "section",
			section: "workspaceOverview",
		});
	});

	test("opens resource handoffs and resets stale surface targets", () => {
		const argo = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "openArgo",
			application: "old-app",
		});
		const resources = navigateWorkspace(argo, {
			type: "openResources",
			namespaces: ["payments", "billing"],
			search: "checkout",
			gitOpsFilter: "checkout",
			healthFilter: "degraded",
			gitOpsFocusApplication: argoApplication,
		});

		expect(resources).toMatchObject({
			viewMode: "resources",
			selectedNode: null,
			targetGitOpsApplication: null,
			targetHelmRelease: null,
			focusedResource: null,
			resourceInitialSearch: "checkout",
			resourceInitialGitOpsFilter: "checkout",
			resourceInitialHealthFilter: "degraded",
			resourceNamespaceOverride: ["payments", "billing"],
			resourceGitOpsFocusApplication: argoApplication,
		});
	});

	test("routes tree and resource selections while clearing handoff state", () => {
		const initial = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "openResources",
			search: "stale",
			gitOpsFilter: "stale",
			healthFilter: "attention",
			gitOpsFocusApplication: argoApplication,
		});
		const incidents = navigateWorkspace(initial, {
			type: "selectNode",
			node: { type: "section", section: "incidents" },
		});
		const selected = navigateWorkspace(incidents, {
			type: "selectResource",
			resource,
			node: {
				type: "kind",
				section: "workloads",
				kind: "Deployment",
				namespace: "payments",
			},
		});

		expect(incidents).toMatchObject({
			viewMode: "incidents",
			initialIncidentFilter: "all",
			resourceInitialSearch: "",
			resourceInitialGitOpsFilter: "",
			resourceInitialHealthFilter: "all",
			resourceNamespaceOverride: null,
			resourceGitOpsFocusApplication: null,
		});
		expect(selected.viewMode).toBe("resources");
		expect(selected.focusedResource).toEqual(resource);
	});

	test("routes settings, Helm, GitOps, and live-session surfaces", () => {
		const initial = createWorkspaceNavigation(workspace);
		const settings = navigateWorkspace(initial, { type: "openSettings" });
		const overview = navigateWorkspace(settings, { type: "closeSettings" });
		const helm = navigateWorkspace(overview, {
			type: "openHelmRelease",
			name: "checkout",
			namespace: "payments",
		});
		const argo = navigateWorkspace(helm, {
			type: "openArgo",
			application: "checkout",
		});
		const portForwards = navigateWorkspace(argo, { type: "openPortForwards" });

		expect(settings).toMatchObject({ viewMode: "settings", selectedNode: null });
		expect(overview).toMatchObject({
			viewMode: "overview",
			selectedNode: { type: "section", section: "workspaceOverview" },
		});
		expect(helm).toMatchObject({
			viewMode: "helm",
			targetHelmRelease: { name: "checkout", namespace: "payments" },
		});
		expect(argo).toMatchObject({
			viewMode: "argo",
			targetHelmRelease: null,
			targetGitOpsApplication: "checkout",
		});
		expect(portForwards).toMatchObject({
			viewMode: "portForwards",
			targetGitOpsApplication: null,
			selectedNode: { type: "section", section: "portForwards" },
		});
	});

	test("encodes focused resource as the restore target", () => {
		const focused = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "focusResource",
			resource,
		});
		const snapshot = workspaceNavigationSnapshot(focused, {
			workspaceId: workspace.id,
			expandedSections: ["section:workloads"],
			detail: null,
			surfaces: null,
		});

		expect(snapshot.focusedResource).toMatchObject({
			kind: "Deployment",
			name: "checkout",
		});
		expect(snapshot.restoreTargetResource).toEqual(snapshot.focusedResource);
	});

	test("updates resource-browser path state through an intent", () => {
		const pathState: PathStateResourceBrowserState = {
			selectedNamespaces: ["payments"],
			selectedKinds: ["Deployment"],
			search: "checkout",
			gitOpsFilter: "",
			healthFilter: "all",
			sortColumn: "name",
			sortDesc: false,
			pageIndex: 0,
			scopeEditorOpen: false,
			collapsedGroups: [],
			topologyMode: "ownership",
			selectedTopologyNodeId: null,
			mapPanelOpen: true,
			tablePanelOpen: true,
		};
		const updated = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "updateResourceBrowserPath",
			pathState,
		});

		expect(updated.resourceBrowserPathState).toEqual(pathState);
	});

	test("ignores an unchanged resource-browser path state", () => {
		const pathState: PathStateResourceBrowserState = {
			selectedNamespaces: ["payments"],
			selectedKinds: ["Deployment"],
			search: "",
			gitOpsFilter: "",
			healthFilter: "all",
			sortColumn: "name",
			sortDesc: false,
			pageIndex: 0,
			scopeEditorOpen: false,
			collapsedGroups: [],
			topologyMode: "ownership",
			selectedTopologyNodeId: null,
			mapPanelOpen: true,
			tablePanelOpen: true,
		};
		const initial = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "updateResourceBrowserPath",
			pathState,
		});
		const unchanged = navigateWorkspace(initial, {
			type: "updateResourceBrowserPath",
			pathState: { ...pathState },
		});

		expect(unchanged).toBe(initial);
	});

	test("clears cluster-bound navigation after a context change", () => {
		const focused = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "selectResource",
			resource,
			node: { type: "kind", section: "workloads", kind: "Deployment" },
		});
		const changed = navigateWorkspace(focused, { type: "changeCluster" });

		expect(changed).toMatchObject({
			viewMode: "resources",
			selectedNode: null,
			focusedResource: null,
			restoreTargetResource: null,
			targetHelmRelease: null,
			targetGitOpsApplication: null,
			resourceGitOpsFocusApplication: null,
		});
	});
});

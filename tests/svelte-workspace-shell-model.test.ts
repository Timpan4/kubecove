import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildSidebarTree,
	extraDiscoveredKinds,
	getResourceBrowserScope,
	getWorkspacePlaceholder,
	getWorkspaceTitle,
	isNamespaceListView,
	treeNodeForResource,
	viewModeForTreeNode,
} from "../src/app/svelte/workspaceShellModel";
import { createWorkspaceRecord } from "../src/lib/workspace-model";
import { SUPPORTED_KINDS } from "../src/lib/types";
import type {
	DiscoveredResourceKind,
	FluxDetectionSummary,
	NamespaceSummary,
	ResourceSummary,
} from "../src/lib/types";
import type { TreeNode, TreeNodeId } from "../src/lib/tree-nav";

const namespaces: NamespaceSummary[] = [
	{ name: "default", age: "1d" },
	{ name: "argocd", age: "2d" },
];

const widgets: DiscoveredResourceKind = {
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: "Widget",
	plural: "widgets",
	namespaced: true,
};

const clusterThings: DiscoveredResourceKind = {
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: "ClusterThing",
	plural: "clusterthings",
	namespaced: false,
};

const deployment: DiscoveredResourceKind = {
	group: "apps",
	version: "v1",
	apiVersion: "apps/v1",
	kind: "Deployment",
	plural: "deployments",
	namespaced: true,
};

function resource(kind: string, namespace: string | null): ResourceSummary {
	return {
		kind,
		cluster: "kind-dev",
		name: kind.toLowerCase(),
		namespace,
		age: "1d",
		health: "healthy",
	};
}

const fluxDetected: FluxDetectionSummary = {
	detected: true,
	kinds: [],
	missingKinds: [],
};

function findNode(nodes: TreeNode[], label: string): TreeNode {
	const match = nodes.find((node) => node.label === label);
	if (!match) throw new Error(`Missing node ${label}`);
	return match;
}

describe("svelte workspace shell model", () => {
	test("sorts CRD-backed custom resource tree additions", () => {
		expect(extraDiscoveredKinds([widgets, deployment, clusterThings])).toEqual([
			clusterThings,
			deployment,
			widgets,
		]);
	});

	test("builds opened-workspace sidebar parity tree from query data", () => {
		const nodes = buildSidebarTree({
			namespaces,
			resourceKinds: [widgets, deployment, clusterThings],
			argoDetected: true,
			fluxDetection: fluxDetected,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});

		expect(nodes.map((node) => node.label)).toEqual([
			"Workspace Overview",
			"Cluster Overview",
			"Namespaces",
			"Workloads",
			"Network",
			"Config",
			"Storage",
			"Custom Resources",
			"GitOps",
			"Helm",
			"Incidents",
			"Port Forwards",
			"RBAC",
		]);
		expect(findNode(nodes, "Namespaces").children?.map((node) => node.label)).toEqual([
			"default",
			"argocd",
		]);
		expect(
			findNode(nodes, "Custom Resources").children?.map((node) => node.label),
		).toEqual(["ClusterThing", "Deployment", "Widget"]);
		expect(findNode(nodes, "GitOps").children?.map((node) => node.label)).toEqual([
			"Argo CD",
			"Flux",
		]);
	});

	test("keeps namespace sidebar children lazy", () => {
		const nodes = buildSidebarTree({
			namespaces,
			resourceKinds: [widgets],
			argoDetected: false,
			fluxDetection: { detected: false, kinds: [], missingKinds: [] },
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});
		const namespace = findNode(nodes, "Namespaces").children?.[0];
		const sidebarSource = readFileSync("src/app/svelte/SidebarTree.svelte", "utf8");
		const nodeSource = readFileSync(
			"src/app/svelte/SidebarTreeNode.svelte",
			"utf8",
		);

		expect(namespace?.label).toBe("default");
		expect(namespace?.children).toBeUndefined();
		expect(sidebarSource).toContain("listPresentCustomResourceKinds");
		expect(sidebarSource).toContain("buildNamespaceTreeNode(node.id.namespace, customResources)");
		expect(sidebarSource).toContain("{getLazyChildren}");
		expect(nodeSource).toContain('node.id.type === "namespace"');
		expect(nodeSource).toContain("getLazyChildren?.(node)");
	});

	test("prewarms custom resources and refreshes topology when CRDs arrive", () => {
		const shellSource = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const browserSource = readFileSync("src/features/resources/ResourceBrowser.svelte", "utf8");

		expect(shellSource).toContain("workspaceCustomResourcePrewarmQuery");
		expect(shellSource).toContain("workspace.scope.namespaces");
		expect(shellSource).toContain("listPresentCustomResourceKinds(");
		expect(browserSource).toContain("topologyCustomResourceKey");
		expect(browserSource).toContain("topologyBaseQueryKey");
		expect(browserSource).toContain("[...topologyBaseQueryKey, topologyCustomResourceKey]");
		expect(browserSource).toContain("JSON.stringify(topologyBaseQueryKey)");
	});

	test("uses disabled placeholders while query-backed tree data loads or fails", () => {
		const loadingTree = buildSidebarTree({
			namespaces: [],
			resourceKinds: [],
			argoDetected: undefined,
			fluxDetection: undefined,
			detectingGitOps: true,
			resourceKindsPending: true,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});
		expect(findNode(loadingTree, "Custom Resources").children?.[0]).toMatchObject({
			label: "Loading custom resources...",
			disabled: true,
		});
		expect(findNode(loadingTree, "GitOps").children?.[0]).toMatchObject({
			label: "Detecting providers...",
			disabled: true,
		});

		const failedTree = buildSidebarTree({
			namespaces: [],
			resourceKinds: [],
			argoDetected: false,
			fluxDetection: { detected: false, kinds: [], missingKinds: [] },
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "boom",
			showUnavailableGitOpsProviders: true,
		});
		expect(findNode(failedTree, "Custom Resources").children?.[0]).toMatchObject({
			label: "Custom resource discovery unavailable",
			description: "boom",
			disabled: true,
		});
		expect(findNode(failedTree, "GitOps").children?.map((node) => node.label)).toEqual([
			"Argo CD",
			"Flux",
		]);
		expect(findNode(failedTree, "GitOps").children?.every((node) => node.disabled)).toBe(
			true,
		);
	});

	test("maps sidebar selections to shell titles and empty descriptions", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
		});
		const overview: TreeNodeId = {
			type: "section",
			section: "workspaceOverview",
		};
		const pods: TreeNodeId = {
			type: "kind",
			section: "workloads",
			kind: "Pod",
		};
		const argoProvider: TreeNodeId = {
			type: "group",
			section: "argo",
			group: "gitops:argo",
		};
		const settingsMode = "settings";

		expect(viewModeForTreeNode(overview)).toBe("overview");
		expect(viewModeForTreeNode(pods)).toBe("resources");
		expect(viewModeForTreeNode(argoProvider)).toBe("argo");
		expect(
			getWorkspaceTitle({
				workspace,
				selectedNode: overview,
				viewMode: "overview",
			}),
		).toBe("Ops");
		expect(
			getWorkspaceTitle({
				workspace,
				selectedNode: pods,
				viewMode: "resources",
			}),
		).toBe("Pod Resources");
		expect(
			getWorkspaceTitle({
				workspace,
				selectedNode: argoProvider,
				viewMode: "argo",
			}),
		).toBe("Argo CD");
		expect(
			getWorkspaceTitle({
				workspace,
				selectedNode: null,
				viewMode: settingsMode,
			}),
		).toBe("Settings");
		expect(
			getWorkspacePlaceholder({
				selectedNode: pods,
				viewMode: "resources",
			}),
		).toBe("Browse live resources for this scope.");
	});

	test("maps selected resources back to resource browser tree nodes", () => {
		expect(treeNodeForResource(resource("Pod", "default"))).toEqual({
			type: "kind",
			section: "workloads",
			namespace: "default",
			kind: "Pod",
		});
		expect(treeNodeForResource(resource("Node", null))).toEqual({
			type: "kind",
			section: "clusterOverview",
			namespace: undefined,
			kind: "Node",
		});
		expect(treeNodeForResource(resource("Widget", "apps"))).toEqual({
			type: "kind",
			section: "discovered",
			namespace: "apps",
			kind: "Widget",
		});
	});

	test("routes only top-level Namespaces to the Svelte namespace list", () => {
		const namespacesSection: TreeNodeId = {
			type: "section",
			section: "namespaces",
		};
		const namespaceChild: TreeNodeId = {
			type: "namespace",
			section: "namespaces",
			namespace: "default",
		};
		const pods: TreeNodeId = {
			type: "kind",
			section: "workloads",
			kind: "Pod",
		};

		expect(
			isNamespaceListView({
				selectedNode: namespacesSection,
				viewMode: "resources",
			}),
		).toBe(true);
		expect(
			isNamespaceListView({
				selectedNode: namespaceChild,
				viewMode: "resources",
			}),
		).toBe(false);
		expect(isNamespaceListView({ selectedNode: pods, viewMode: "resources" })).toBe(
			false,
		);
		expect(
			isNamespaceListView({
				selectedNode: namespacesSection,
				viewMode: "overview",
			}),
		).toBe(false);
	});
	test("resolves queryable Svelte resource-browser scopes from sidebar selections", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
			kinds: ["Pod", "Deployment"],
		});
		const namespacesSection: TreeNodeId = {
			type: "section",
			section: "namespaces",
		};
		const namespaceChild: TreeNodeId = {
			type: "namespace",
			section: "namespaces",
			namespace: "argocd",
		};
		const topLevelPod: TreeNodeId = {
			type: "kind",
			section: "workloads",
			kind: "Pod",
		};
		const clusterNode: TreeNodeId = {
			type: "kind",
			section: "clusterOverview",
			kind: "Node",
		};

		expect(
			getResourceBrowserScope({
				workspace,
				selectedNode: namespacesSection,
				viewMode: "resources",
			}).canQuery,
		).toBe(false);
		expect(
			getResourceBrowserScope({
				workspace,
				selectedNode: namespaceChild,
				viewMode: "resources",
			}),
		).toEqual({
			canQuery: true,
			namespaces: ["argocd"],
			kinds: [...SUPPORTED_KINDS],
		});
		expect(
			getResourceBrowserScope({
				workspace,
				selectedNode: topLevelPod,
				viewMode: "resources",
			}),
		).toEqual({ canQuery: true, namespaces: ["default"], kinds: ["Pod"] });
		expect(
			getResourceBrowserScope({
				workspace,
				selectedNode: clusterNode,
				viewMode: "resources",
			}),
		).toEqual({ canQuery: true, namespaces: [], kinds: ["Node"] });
	});

	test("Svelte shell clears resource handoff filters when navigating away", () => {
		const source = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");

		expect(source).toContain(
			"initialGitOpsFilter = workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter ?? \"\"",
		);
		const selectNodeStart = source.indexOf("function selectNode");
		const selectNodeEnd = source.indexOf("function selectResource", selectNodeStart);
		const selectNodeBody = source.slice(selectNodeStart, selectNodeEnd);
		expect(selectNodeBody).toContain('resourceInitialSearch = ""');
		expect(selectNodeBody).toContain('resourceInitialGitOpsFilter = ""');
		expect(selectNodeBody).toContain("resourceNamespaceOverride = null");
	});

	test("Svelte workspace chrome keeps platform-aware command palette hint", () => {
		const source = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");

		expect(source).toContain('const SEARCH_SHORTCUT_HINT = IS_MAC ? "⌘K" : "Ctrl K"');
		expect(source).toContain("{SEARCH_SHORTCUT_HINT}");
		expect(source).toContain('aria-label="Search views, namespaces, and resources"');
		expect(source).toContain("commandOpen = true");
	});

	test("Svelte workspace chrome uses real cluster selector", () => {
		const shell = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const selector = readFileSync("src/components/ClusterSelector.svelte", "utf8");

		expect(shell).toContain('import ClusterSelector from "@/components/ClusterSelector.svelte"');
		expect(shell).toContain("<ClusterSelector");
		expect(shell).toContain("value={workspace.scope.clusterContext}");
		expect(shell).toContain("onClusterChange={changeClusterContext}");
		expect(shell).toContain("workspaceStore.updateWorkspace(workspace.id");
		expect(shell).toContain("clusterContexts: [clusterContext]");
		expect(shell).toContain("namespaces: []");
		expect(shell).toContain('viewMode = "resources"');
		expect(selector).toContain("queryKeys.kubeContexts(kubeconfigSourceKey)");
		expect(selector).toContain("listKubeContexts(client, kubeconfigSourceKey)");
		expect(selector).toContain('aria-labelledby="cluster-select-label"');
		expect(selector).toContain('placeholder="Select a context..."');
		expect(selector).toContain("contextsQuery.refetch()");
	});

	test("Svelte workspace shell owns the selected resource inspector", () => {
		const shell = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");

		expect(shell).toContain('import DetailPanelFrame from "./DetailPanelFrame.svelte"');
		expect(shell).toContain(
			'import ResourceDetailPanel from "@/features/resource-detail/ResourceDetailPanel.svelte"',
		);
	expect(shell).toContain(
		"const resourceInspectorOpen = $derived(focusedResource !== null)",
	);
	expect(shell).toContain('const resourceInspectorSizeKey = $derived(viewMode === "argo" ? "gitops" : "resource")');
	expect(shell).toContain('const resourceInspectorDefaultSize = $derived(viewMode === "argo" ? 30 : 40)');
	expect(shell).toContain('const resourceInspectorMinSize = $derived(viewMode === "argo" ? 25 : 33)');
	expect(shell).toContain("detailOpen={resourceInspectorOpen}");
	expect(shell).toContain("sizeKey={resourceInspectorSizeKey}");
	expect(shell).toContain("selectedResource={focusedResource}");
	expect(shell).toContain("focusedResource = resource;");
	expect(shell).toContain("focusedResource = null;");
	expect(shell).toContain("{#key resourceSelectionKey(focusedResource)}");
	expect(shell).toContain('aria-label="Close resource details"');
	expect(shell).not.toContain("Back to workspaces");
});

	test("Svelte Argo shortcuts preserve target application selection", () => {
		const overview = readFileSync("src/features/workspaces/WorkspaceOverview.svelte", "utf8");
		const shell = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const surfaces = readFileSync("src/app/svelte/AppSurfaces.svelte", "utf8");

		expect(overview).toContain("onOpenArgo(shortcut.argoApp)");
		expect(shell).toContain("let targetGitOpsApplication");
		expect(shell).toContain(
			"onTargetGitOpsApplicationResolved={clearTargetGitOpsApplication}",
		);
		expect(surfaces).toContain("targetGitOpsApplication");
		expect(surfaces).toContain('item.type === "argoApp"');
	});

	test("Svelte workspace overview summarizes Flux in the GitOps card", () => {
		const overview = readFileSync("src/features/workspaces/WorkspaceOverview.svelte", "utf8");

		expect(overview).toContain("detectFlux");
		expect(overview).toContain("listFluxResources");
		expect(overview).toContain("queryKeys.fluxDetect(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(overview).toContain("queryKeys.fluxResources(");
		expect(overview).toContain("const fluxRows = $derived(");
		expect(overview).toContain("<CardTitle>GitOps</CardTitle>");
		expect(overview).toContain("Flux resources");
		expect(overview).not.toContain("<CardTitle>Argo CD</CardTitle>");
	});
});

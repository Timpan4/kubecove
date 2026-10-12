import { filterCustomResourceKinds } from "@/components/sidebar-tree-helpers";
import type { DiscoveredResourceKind } from "@/lib/types";
import type { TreeNode, TreeNodeId } from "@/lib/tree-nav";
import {
	appendPresentCustomResourceKinds,
	buildSidebarTree,
	collectSelectedAncestorIds,
	resourceBrowserAvailableKinds,
	sidebarSelectionNode,
	type SidebarTreeGroup,
} from "./workspaceShellModel";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toEqual<Expected>(expected: Expected): void;
};

const widget: DiscoveredResourceKind = {
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: "Widget",
	plural: "widgets",
	shortNames: ["wdg"],
	namespaced: true,
};

const helmGroups: SidebarTreeGroup[] = [
	{
		id: "cluster",
		nodes: [
			{
				id: { type: "section", section: "helm" },
				label: "Helm",
				children: [
					{ id: { type: "kind", section: "helm", kind: "Releases" }, label: "Releases" },
				],
			},
			{ id: { type: "section", section: "rbac" }, label: "RBAC" },
		],
	},
];

function treeNodes(groups: SidebarTreeGroup[]): TreeNode[] {
	return groups.flatMap((group) => group.nodes);
}

describe("Custom Resources tree model", () => {
	test("labels CRD-backed catalog as Custom Resources", () => {
		const groups = buildSidebarTree({
			namespaces: [],
			resourceKinds: [widget],
			argoDetected: false,
			fluxDetection: undefined,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});

		const customResources = groups
			.find((group) => group.id === "scope")
			?.nodes.find((node) => node.id.section === "discovered");

		expect(customResources?.label).toBe("Custom Resources");
		expect(customResources?.children?.[0]?.label).toBe("example.com");
		expect(customResources?.children?.[0]?.children?.[0]?.label).toBe("Widget");
	});

	test("omits Custom Resources when disabled", () => {
		const groups = buildSidebarTree({
			namespaces: [],
			resourceKinds: [widget],
			argoDetected: false,
			fluxDetection: undefined,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
			showCustomResources: false,
		});

		expect(treeNodes(groups).some((node) => node.id.section === "discovered")).toBe(false);
	});

	test("filters custom resources by CRD names and API metadata", () => {
		expect(filterCustomResourceKinds([widget], "WDG")).toEqual([widget]);
		expect(filterCustomResourceKinds([widget], "example.com/v1")).toEqual([widget]);
		expect(filterCustomResourceKinds([widget], "missing")).toEqual([]);
	});

	test("shows an explicit empty search result", () => {
		const groups = buildSidebarTree({
			namespaces: [],
			resourceKinds: [widget],
			argoDetected: false,
			fluxDetection: undefined,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
			customResourceSearch: "missing",
		});

		expect(treeNodes(groups).find((node) => node.id.section === "discovered")?.children?.[0]?.label).toBe(
			"No custom resources match search",
		);
	});

	test("appends present custom resources without duplicates", () => {
		expect(appendPresentCustomResourceKinds(["Pod", widget], [widget])).toEqual([
			"Pod",
			widget,
		]);
	});

	test("keeps a fixed namespace kind scope from adding discovered kinds", () => {
		expect(resourceBrowserAvailableKinds(["Pod"], [widget], true)).toEqual([
			"Pod",
		]);
	});
});

describe("Sidebar groups", () => {
	test("resolves external custom-resource and GitOps kinds to their sidebar rows", () => {
		const groups = buildSidebarTree({
			namespaces: [], resourceKinds: [widget], argoDetected: true,
			fluxDetection: undefined, detectingGitOps: false, resourceKindsPending: false,
			resourceKindsError: "", showUnavailableGitOpsProviders: false,
		});
		const custom = sidebarSelectionNode({ type: "kind", section: "discovered", namespace: "default", kind: "Widget" }, groups);
		expect(custom?.kind).toBe("example.com/v1/widgets/Widget");
		expect([...collectSelectedAncestorIds(groups, custom)]).toEqual(["section::discovered", "group::discovered::example.com"]);
		const argo = sidebarSelectionNode({ type: "kind", section: "argo", kind: "Argo CD Applications" }, groups);
		expect(argo?.group).toBe("gitops:argo");
		expect([...collectSelectedAncestorIds(groups, argo)]).toEqual(["section::argo", "group::argo::gitops:argo"]);
		expect(sidebarSelectionNode(custom, groups)).toEqual(custom);
	});

	test("reveals the section without guessing between identical custom-resource names", () => {
		const groups = buildSidebarTree({
			namespaces: [], resourceKinds: [widget, { ...widget, group: "other.com", apiVersion: "other.com/v1" }],
			argoDetected: false, fluxDetection: undefined, detectingGitOps: false,
			resourceKindsPending: false, resourceKindsError: "", showUnavailableGitOpsProviders: false,
		});
		const custom = sidebarSelectionNode({ type: "kind", section: "discovered", kind: "Widget" }, groups);
		expect(custom).toEqual({ type: "section", section: "discovered" });
		expect([...collectSelectedAncestorIds(groups, custom)]).toEqual([]);
		const filtered = sidebarSelectionNode({ type: "kind", section: "discovered", kind: "HiddenKind" }, groups);
		expect(filtered).toEqual({ type: "section", section: "discovered" });
	});

	test("matches external resource selections without changing their namespace scope", () => {
		const external: TreeNodeId = { type: "kind", section: "helm", namespace: "default", kind: "Releases" };
		const sidebar = sidebarSelectionNode(external);
		expect([...collectSelectedAncestorIds(helmGroups, sidebar)]).toEqual(["section::helm"]);
		expect(sidebar?.namespace).toBe(undefined);
		expect(external.namespace).toBe("default");
		const namespaceKind: TreeNodeId = { type: "kind", section: "namespaces", namespace: "default", group: "Workloads", kind: "Pod" };
		expect(sidebarSelectionNode(namespaceKind)).toEqual(namespaceKind);
		expect(sidebarSelectionNode(null)).toBe(null);
	});

	test("lists namespaces as top-level rows without a Namespaces section row", () => {
		const groups = buildSidebarTree({
			namespaces: [{ name: "default", age: "1d" }],
			resourceKinds: [],
			argoDetected: false,
			fluxDetection: undefined,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});

		expect(groups.map((group) => group.id)).toEqual([
			"workspace",
			"cluster",
			"scope",
			"namespaces",
		]);
		expect(
			groups.find((group) => group.id === "namespaces")?.nodes.map((node) => node.label),
		).toEqual(["default"]);
		expect(
			treeNodes(groups).some(
				(node) => node.id.type === "section" && node.id.section === "namespaces",
			),
		).toBe(false);
	});

	test("returns no ancestors for null, missing, or top-level selections", () => {
		expect([...collectSelectedAncestorIds(helmGroups, null)]).toEqual([]);
		expect([
			...collectSelectedAncestorIds(helmGroups, { type: "section", section: "missing" }),
		]).toEqual([]);
		expect([
			...collectSelectedAncestorIds(helmGroups, { type: "section", section: "rbac" }),
		]).toEqual([]);
	});

	test("returns the ancestors of a nested selection", () => {
		expect([
			...collectSelectedAncestorIds(helmGroups, {
				type: "kind",
				section: "helm",
				kind: "Releases",
			}),
		]).toEqual(["section::helm"]);
	});

	test("walks lazily loaded children to find ancestors", () => {
		const groups: SidebarTreeGroup[] = [
			{
				id: "namespaces",
				nodes: [
					{
						id: { type: "namespace", section: "namespaces", namespace: "default" },
						label: "default",
					},
				],
			},
		];
		const pod: TreeNodeId = {
			type: "kind",
			section: "namespaces",
			namespace: "default",
			group: "Workloads",
			kind: "Pod",
		};
		const getLazyChildren = (node: TreeNode): TreeNode[] | undefined =>
			node.id.type === "namespace"
				? [
						{
							id: {
								type: "group",
								section: "namespaces",
								namespace: "default",
								group: "Workloads",
							},
							label: "Workloads",
							children: [{ id: pod, label: "Pod" }],
						},
					]
				: undefined;

		expect([...collectSelectedAncestorIds(groups, pod, getLazyChildren)]).toEqual([
			"namespace::namespaces::default",
			"group::namespaces::default::Workloads",
		]);
		expect([...collectSelectedAncestorIds(groups, pod)]).toEqual(["namespace::namespaces::default"]);
		expect([...collectSelectedAncestorIds(groups, {
			...pod,
			group: widget.group,
			kind: "example.com/v1/widgets/Widget",
			resourceKind: widget,
		}, () => [])]).toEqual(["namespace::namespaces::default"]);
	});
});

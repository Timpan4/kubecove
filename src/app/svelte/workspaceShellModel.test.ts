import type { DiscoveredResourceKind } from "@/lib/types";
import {
	appendPresentCustomResourceKinds,
	buildSidebarTree,
	filterCustomResourceKinds,
	GITOPS_RESOURCE_KINDS,
	resourceBrowserAvailableKinds,
} from "./workspaceShellModel";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
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

describe("Custom Resources tree model", () => {
	test("labels CRD-backed catalog as Custom Resources", () => {
		const nodes = buildSidebarTree({
			namespaces: [],
			resourceKinds: [widget],
			argoDetected: false,
			fluxDetection: undefined,
			detectingGitOps: false,
			resourceKindsPending: false,
			resourceKindsError: "",
			showUnavailableGitOpsProviders: false,
		});

		const customResources = nodes.find((node) => node.id.section === "discovered");

		expect(customResources?.label).toBe("Custom Resources");
		expect(customResources?.children?.[0]?.label).toBe("example.com");
		expect(customResources?.children?.[0]?.children?.[0]?.label).toBe("Widget");
	});

	test("omits Custom Resources when disabled", () => {
		const nodes = buildSidebarTree({
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

		expect(nodes.some((node) => node.id.section === "discovered")).toBe(false);
	});

	test("filters custom resources by CRD names and API metadata", () => {
		expect(filterCustomResourceKinds([widget], "WDG")).toEqual([widget]);
		expect(filterCustomResourceKinds([widget], "example.com/v1")).toEqual([widget]);
		expect(filterCustomResourceKinds([widget], "missing")).toEqual([]);
	});

	test("shows an explicit empty search result", () => {
		const nodes = buildSidebarTree({
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

		expect(nodes.find((node) => node.id.section === "discovered")?.children?.[0]?.label).toBe(
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

	test("includes CRD definition objects in GitOps resource scopes", () => {
		expect(GITOPS_RESOURCE_KINDS.includes("CustomResourceDefinition")).toBe(true);
	});

	test("keeps cluster-scoped native kinds in GitOps resource scopes", () => {
		expect(GITOPS_RESOURCE_KINDS.includes("Node")).toBe(true);
		expect(GITOPS_RESOURCE_KINDS.includes("StorageClass")).toBe(true);
		expect(GITOPS_RESOURCE_KINDS.includes("PersistentVolume")).toBe(true);
	});
});

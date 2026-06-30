import type { DiscoveredResourceKind } from "@/lib/types";
import {
	GITOPS_RESOURCE_KINDS,
	appendPresentCustomResourceKinds,
	buildSidebarTree,
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
		expect(customResources?.children?.[0]?.label).toBe("Widget");
	});

	test("appends present custom resources without duplicates", () => {
		expect(appendPresentCustomResourceKinds(["Pod", widget], [widget])).toEqual([
			"Pod",
			widget,
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

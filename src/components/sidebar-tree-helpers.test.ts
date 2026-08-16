import type { DiscoveredResourceKind } from "@/lib/types";
import { buildNamespaceTreeNode, toggleTreeNode } from "./sidebar-tree-helpers";

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

describe("namespace custom resources", () => {
	test("expands a tree node without a selection callback", () => {
		const expanded: string[] = [];

		toggleTreeNode(true, "namespace::namespaces::payments", (id) => {
			expanded.push(id);
		});

		expect(expanded).toEqual(["namespace::namespaces::payments"]);
	});

	test("omits custom resources group when none are present", () => {
		const node = buildNamespaceTreeNode("payments", []);

		expect(node.children?.some((child) => child.label === "Custom Resources")).toBe(false);
	});

	test("shows present custom resources", () => {
		const node = buildNamespaceTreeNode("payments", [widget]);

		const group = node.children?.find((child) => child.label === "Custom Resources");

		expect(group?.children?.[0]?.label).toBe("Widget");
	});
});

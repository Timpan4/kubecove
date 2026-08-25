import type { DiscoveredResourceKind } from "@/lib/types";
import { buildNamespaceTreeNode } from "./sidebar-tree-helpers";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
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

const gadget: DiscoveredResourceKind = {
	group: "another.example.com",
	version: "v1",
	apiVersion: "another.example.com/v1",
	kind: "Gadget",
	plural: "gadgets",
	shortNames: ["gdt"],
	namespaced: true,
};

describe("namespace custom resources", () => {
	test("omits custom resources group when none are present", () => {
		const node = buildNamespaceTreeNode("payments", []);

		expect(node.children?.some((child) => child.label === "Custom Resources")).toBe(false);
	});

	test("shows present custom resources", () => {
		const node = buildNamespaceTreeNode("payments", [widget, gadget]);

		const group = node.children?.find((child) => child.label === "Custom Resources");

		expect(group?.children?.[0]?.label).toBe("another.example.com");
		expect(group?.selectable).toBe(false);
		expect(group?.children?.[0]?.selectable).toBe(false);
		expect(group?.children?.[1]?.label).toBe("example.com");
		expect(group?.children?.[1]?.children?.[0]?.label).toBe("Widget");
		expect(group?.children?.[1]?.children?.[0]?.id.resourceKind).toBe(widget);
	});
});

import { describe, expect, test } from "bun:test";
import {
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
} from "../src/components/sidebar-tree-helpers";
import type { TreeNode } from "../src/lib/tree-nav";
import type { DiscoveredResourceKind } from "../src/lib/types";

function widgetKind(index: number): DiscoveredResourceKind {
	return {
		group: "example.com",
		version: "v1",
		apiVersion: "example.com/v1",
		kind: `Widget${index}`,
		plural: `widgets${index}`,
		namespaced: true,
	};
}

function countTreeNodes(nodes: TreeNode[]): number {
	let count = 0;
	const stack = [...nodes];
	while (stack.length > 0) {
		const node = stack.pop();
		if (!node) continue;
		count += 1;
		if (node.children) stack.push(...node.children);
	}
	return count;
}

describe("sidebar namespace tree helpers", () => {
	test("keeps namespace rows shallow until expanded", () => {
		const shallow = buildShallowNamespaceTreeNode("payments");

		expect(shallow).toEqual({
			id: { type: "namespace", section: "namespaces", namespace: "payments" },
			label: "payments",
		});
	});

	test("builds deep namespace children only for expanded namespaces", () => {
		const deep = buildNamespaceTreeNode(
			"payments",
			Array.from({ length: 10 }, (_, index) => widgetKind(index)),
		);

		expect(countTreeNodes([deep])).toBeGreaterThan(20);
		expect(deep.children?.at(-1)?.label).toBe("Custom Resources");
		expect(deep.children?.at(-1)?.children?.[0]?.label).toBe("example.com");
		expect(deep.children?.at(-1)?.children?.[0]?.children?.length).toBe(10);
	});
});

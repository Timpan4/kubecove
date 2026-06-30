import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildNamespaceTreeNode,
	buildShallowNamespaceTreeNode,
} from "../src/components/sidebar-tree-helpers";
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

function countTreeNodes(nodes: unknown[]): number {
	let count = 0;
	const stack = [...nodes] as Array<{ children?: unknown[] }>;
	while (stack.length > 0) {
		const node = stack.pop();
		if (!node) continue;
		count += 1;
		if (Array.isArray(node.children)) {
			stack.push(...(node.children as Array<{ children?: unknown[] }>));
		}
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
		expect(deep.children?.at(-1)?.children?.length).toBe(10);
	});

	test("uses the same selection path for sidebar rows and chevrons", () => {
		const source = readFileSync("src/app/svelte/SidebarTreeNode.svelte", "utf8");
		const toggleStart = source.indexOf("function toggleNode");
		const toggleEnd = source.indexOf("function handleKeydown", toggleStart);
		const toggleSource = source.slice(toggleStart, toggleEnd);

		expect(toggleSource).toContain("onNodeSelect(node.id)");
		expect(toggleSource).toContain("onSectionToggle(id)");
		expect(source).toContain("onclick={selectNode}");
		expect(source).toContain("onclick={toggleNode}");
	});
});

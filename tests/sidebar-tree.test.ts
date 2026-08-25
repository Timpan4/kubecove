import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
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

	test("expands chevrons without replacing the selected scope", () => {
		const source = readFileSync("src/app/svelte/SidebarTreeNode.svelte", "utf8");
		const selectStart = source.indexOf("function selectNode");
		const toggleStart = source.indexOf("function toggleNode");
		const selectSource = source.slice(selectStart, toggleStart);
		const toggleEnd = source.indexOf("function handleKeydown", toggleStart);
		const toggleSource = source.slice(toggleStart, toggleEnd);

		expect(selectSource).toContain("if (node.selectable !== false) onNodeSelect(node.id)");
		expect(selectSource).toContain("if (hasChildren) onSectionToggle(id)");
		expect(toggleSource).not.toContain("onNodeSelect(node.id)");
		expect(toggleSource).toContain("onSectionToggle(id)");
		expect(source).toContain("onclick={selectNode}");
		expect(source).toContain("onclick={toggleNode}");
	});

	test("gates custom resource presence queries when disabled", () => {
		const sidebar = readFileSync("src/app/svelte/SidebarTree.svelte", "utf8");
		const shell = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");

		expect(sidebar).toContain(
			"enabled: showCustomResources && Boolean(clusterContext) && sourceReady",
		);
		expect(sidebar).toContain("if (!showCustomResources) return node.children");
		expect(shell).toContain(
			"enabled: showCustomResources && Boolean(workspace.scope.clusterContext)",
		);
		expect(shell).toContain("showCustomResources &&");
		expect(shell).toContain("includePresentCustomResources");
	});
});

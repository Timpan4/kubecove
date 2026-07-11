import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { parse } from "svelte/compiler";

type AstNode = Record<string, unknown>;

describe("GitOps surface component contract", () => {
	test("connects selected application path state through a bindable feature prop", () => {
		const surface = parseComponent("src/features/gitops/GitOpsSurface.svelte");
		const selectionProperty = findNode(
			surface,
			(node) =>
				node.type === "Property" &&
				asNode(node.key)?.name === "selectedGitOpsItem" &&
				asNode(asNode(asNode(node.value)?.right)?.callee)?.name === "$bindable",
		);
		expect(selectionProperty).not.toBeNull();

		const app = parseComponent("src/app/svelte/AppSurfaces.svelte");
		const gitOpsSurface = findNode(
			app,
			(node) => node.type === "Component" && node.name === "GitOpsSurface",
		);
		const selectionBinding = nodeArray(gitOpsSurface?.attributes).find(
			(attribute) =>
				attribute.type === "BindDirective" && attribute.name === "selectedGitOpsItem",
		);
		expect(asNode(selectionBinding?.expression)?.name).toBe("selectedGitOpsItem");
	});
});

function parseComponent(path: string): unknown {
	return parse(readFileSync(path, "utf8"), { modern: true });
}

function asNode(value: unknown): AstNode | null {
	return value !== null && typeof value === "object" && !Array.isArray(value)
		? (value as AstNode)
		: null;
}

function nodeArray(value: unknown): AstNode[] {
	return Array.isArray(value) ? value.flatMap((item) => asNode(item) ?? []) : [];
}

function findNode(root: unknown, matches: (node: AstNode) => boolean): AstNode | null {
	const node = asNode(root);
	if (!node) return null;
	if (matches(node)) return node;
	for (const child of Object.values(node)) {
		const found = Array.isArray(child)
			? child.map((item) => findNode(item, matches)).find(Boolean)
			: findNode(child, matches);
		if (found) return found as AstNode;
	}
	return null;
}

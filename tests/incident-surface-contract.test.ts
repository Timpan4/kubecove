import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { parse } from "svelte/compiler";

type AstValue = AstNode | AstValue[] | string | number | boolean | null;
type AstNode = Record<string, AstValue>;

function parseComponent(path: string): AstNode {
	// SAFETY: Svelte compiler's modern AST is traversed here only as nested JSON-like nodes and scalar fields.
	return parse(readFileSync(path, "utf8"), { modern: true }) as AstNode;
}

describe("Incident surface component contract", () => {
	test("connects external filter resets through a bindable feature prop", () => {
		const surface = parseComponent("src/features/incidents/IncidentSurface.svelte");
		const filterProperty = findNode(
			surface,
			(node) =>
				node.type === "Property" &&
				asNode(node.key)?.name === "incidentFilter" &&
				asNode(asNode(asNode(node.value)?.right)?.callee)?.name === "$bindable",
		);

		expect(filterProperty).not.toBeNull();

		const app = parseComponent("src/app/svelte/AppSurfaces.svelte");
		const incidentSurface = findNode(
			app,
			(node) => node.type === "Component" && node.name === "IncidentSurface",
		);
		const filterBinding = nodeArray(incidentSurface?.attributes).find(
			(attribute) =>
				attribute.type === "BindDirective" && attribute.name === "incidentFilter",
		);

		expect(asNode(filterBinding?.expression)?.name).toBe("incidentFilter");
	});
});

function asNode(value: AstValue): AstNode | null {
	if (Array.isArray(value) || !(value instanceof Object)) return null;
	// SAFETY: non-array objects in the compiler AST are nodes with scalar, node, or node-array fields.
	return value as AstNode;
}

function nodeArray(value: AstValue | undefined): AstNode[] {
	return Array.isArray(value) ? value.flatMap((item) => asNode(item) ?? []) : [];
}

function findNode(root: AstValue, matches: (node: AstNode) => boolean): AstNode | null {
	const node = asNode(root);
	if (!node) return null;
	if (matches(node)) return node;
	for (const child of Object.values(node)) {
		const found = Array.isArray(child)
			? child.map((item) => findNode(item, matches)).find(Boolean)
			: findNode(child, matches);
	if (found) return found;
	}
	return null;
}

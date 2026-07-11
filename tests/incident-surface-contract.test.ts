import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { parse } from "svelte/compiler";

type AstNode = Record<string, unknown>;

function parseComponent(path: string): unknown {
	return parse(readFileSync(path, "utf8"), { modern: true });
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

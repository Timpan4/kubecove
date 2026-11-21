import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { parseSync, Visitor } from "vite";

test("every server command has an exported Svelte invoke wrapper", () => {
	const contract = readFileSync(new URL("../src-tauri/crates/models/src/command_contract.rs", import.meta.url), "utf8");
	const entries = /\$adapter!\s*\{([^}]*)\}/.exec(contract)?.[1];
	if (!entries?.trim()) throw new Error("Cannot read command_contract!'s adapter entries");
	const required = new Set<string>();
	for (const declaration of entries.replace(/\/\/[^\n]*/g, "").split(";").filter((entry) => entry.trim())) {
		const entry = /^\s*(both|remote_intercepted|desktop_only)\s+(?:async|sync)\s+fn\s+(\w+)\s*\(/.exec(declaration);
		if (!entry) throw new Error(`Unrecognized command declaration: ${declaration.trim()}`);
		if (entry[1] !== "desktop_only") required.add(entry[2]);
	}

	const wired = new Set<string>();
	const visitor = new Visitor({
		CallExpression({ callee, arguments: [command] }) {
			if (callee.type === "MemberExpression" && !callee.computed && callee.property.type === "Identifier" && callee.property.name === "invoke"
				&& callee.object.type === "Identifier" && callee.object.name === "client"
				&& command?.type === "Literal" && typeof command.value === "string") wired.add(command.value);
		},
	});
	// Follow the public wrapper barrel. Mock dispatch tables, comments and private functions cannot satisfy the check.
	const modules = new Set([new URL("../src/lib/tauri.ts", import.meta.url).href]);
	for (const url of modules) {
		const file = new URL(url);
		const parsed = parseSync(file.pathname, readFileSync(file, "utf8"));
		expect(parsed.errors).toEqual([]);
		for (const node of parsed.program.body) {
			if (node.type === "ExportAllDeclaration") modules.add(new URL(`${node.source.value}.ts`, file).href);
		}
		visitor.visit({ ...parsed.program, body: parsed.program.body.filter((node) =>
			node.type === "ExportNamedDeclaration" && node.declaration?.type === "FunctionDeclaration") });
	}
	expect([...required].filter((command) => !wired.has(command))).toEqual([]);
});

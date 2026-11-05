import { describe, expect, test } from "bun:test";
import { readdir, readFile } from "node:fs/promises";
import { ACTIONS, loadContract, parseContract, requireViewCoverage, validateReports } from "./contract";
import { VIEWS } from "./visual/views";

describe("equal GPUI and Svelte tests", () => {
	test("every native view has a shared scenario", async () => {
		requireViewCoverage(await loadContract(), VIEWS);
		expect(() => requireViewCoverage({ scenarios: [] }, ["overview"])).toThrow("overview");
	});
	test("both drivers handle every shared action", async () => {
		const web = await readFile("e2e/parity/web.ts", "utf8");
		const directory = "src-tauri/gpui/tests/parity";
		const modules = (await readdir(directory)).filter((name) => name.endsWith(".rs"));
		const gpui = (await Promise.all(["src-tauri/gpui/tests/parity.rs", ...modules.map((name) => `${directory}/${name}`)].map((path) => readFile(path, "utf8")))).join("\n");
		const unhandled = ACTIONS.flatMap((action) => [
			...(web.includes(`case "${action}"`) ? [] : [`web ${action}`]),
			...(new RegExp(`"${action}"(?:\\s*\\|\\s*"[^"]+")*\\s*=>`).test(gpui) ? [] : [`gpui ${action}`]),
		]);
		expect(unhandled).toEqual([]);
	});
});

const contract = { fixture: "browser-mocks", context: "mock-dev", scenarios: [{ id: "one", title: "One", steps: [{ action: "openResources" }] }] };
const report = (driver: string, scenarios: unknown[]) => ({ driver, scenarios });
const passed = { id: "one", status: "passed" };

describe("parity input boundaries", () => {
	test("rejects malformed contracts, IDs and steps", () => {
		for (const value of [null, {}, { ...contract, scenarios: [] }, { ...contract, scenarios: [{ ...contract.scenarios[0], id: "" }] }, { ...contract, scenarios: [...contract.scenarios, ...contract.scenarios] }, { ...contract, scenarios: [{ ...contract.scenarios[0], steps: [{ action: "unknown" }] }] }]) {
			expect(() => parseContract(value)).toThrow();
		}
	});
	test("sidebar actions require a row target", () => {
		for (const action of ["sidebarToggle", "sidebarSelect", "expectSidebarRow"]) {
			expect(() => parseContract({ ...contract, scenarios: [{ ...contract.scenarios[0], steps: [{ action, path: [], visible: true, selected: false }] }] })).toThrow();
		}
	});
	test("requires one passing result per declared scenario from each driver", () => {
		const parsed = parseContract(contract);
		validateReports(parsed, [report("web", [passed]), report("gpui", [passed])]);
		for (const reports of [[], [report("web", [passed])], [report("web", [passed]), report("web", [passed])], [report("web", [passed]), report("gpui", [])], [report("web", [passed, passed]), report("gpui", [passed])], [report("web", [passed]), report("gpui", [passed, { id: "extra", status: "passed" }])], [report("web", [passed]), report("gpui", [{ id: "one", status: "skipped" }])], [report("web", [passed]), report("gpui", [{ id: "one", status: "failed" }])], [report("web", [passed]), undefined]]) {
			expect(() => validateReports(parsed, reports)).toThrow();
		}
	});
});

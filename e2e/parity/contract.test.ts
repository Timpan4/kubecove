import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { ACTIONS, loadContract, parseContract, requireViewCoverage, validateReports } from "./contract";
import { VIEWS } from "./visual/views";

describe("equal GPUI and Svelte tests", () => {
	test("every native view has a shared scenario", async () => {
		requireViewCoverage(await loadContract(), VIEWS);
		expect(() => requireViewCoverage({ scenarios: [] }, ["overview"])).toThrow("overview");
	});
	test("both drivers handle every shared action", async () => {
		const web = await readFile("e2e/parity/web.ts", "utf8");
		const gpui = await readFile("src-tauri/gpui/tests/parity.rs", "utf8");
		const unhandled = ACTIONS.flatMap((action) => [
			...(web.includes(`case "${action}"`) ? [] : [`web ${action}`]),
			...(gpui.includes(`"${action}" =>`) ? [] : [`gpui ${action}`]),
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
	test("requires one passing result per declared scenario from each driver", () => {
		const parsed = parseContract(contract);
		validateReports(parsed, [report("web", [passed]), report("gpui", [passed])]);
		for (const reports of [[], [report("web", [passed])], [report("web", [passed]), report("web", [passed])], [report("web", [passed]), report("gpui", [])], [report("web", [passed, passed]), report("gpui", [passed])], [report("web", [passed]), report("gpui", [passed, { id: "extra", status: "passed" }])], [report("web", [passed]), report("gpui", [{ id: "one", status: "skipped" }])], [report("web", [passed]), report("gpui", [{ id: "one", status: "failed" }])], [report("web", [passed]), undefined]]) {
			expect(() => validateReports(parsed, reports)).toThrow();
		}
	});
});

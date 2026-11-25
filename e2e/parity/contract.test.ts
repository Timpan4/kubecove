import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { mcpMockStatus } from "../../src/lib/mcp-dev-mocks";
import { loadContract, parseContract, requireActionCoverage, requireRegisteredActions, requireViewCoverage, validateReports } from "./contract";
import { mergeActionMaps } from "./action-maps";
import { VIEWS } from "./visual/views";

describe("equal GPUI and Svelte tests", () => {
	test("every native view has a shared scenario", async () => {
		requireViewCoverage(await loadContract(), VIEWS);
		expect(() => requireViewCoverage({ scenarios: [] }, ["overview"])).toThrow("overview");
	});
	test("every action is exercised by the shared scenarios", async () => {
		requireActionCoverage(await loadContract());
		expect(() => requireActionCoverage(parseContract(contract))).toThrow("typeSettingsField");
	});
	// The web driver reads its tool count from the mock, and GPUI from the Rust registry.
	test("the web MCP mock lists the Rust tool registry", async () => {
		const registry = await Bun.file(resolve(import.meta.dir, "../../rust/crates/mcp/src/tools/mod.rs")).text();
		const names = [...registry.matchAll(/^\s*[A-Z]\w* => "([a-z_]+)",/gm)].map((match) => match[1]);
		expect(names.length).toBeGreaterThan(0);
		expect(mcpMockStatus().tools).toEqual(names);
	});
});

const contract = { fixture: "browser-mocks", context: "mock-dev", scenarios: [{ id: "one", title: "One", steps: [{ action: "openResources" }] }] };
const report = (driver: string, scenarios: unknown[]) => ({ driver, scenarios });
const passed = { id: "one", status: "passed" };
const withStep = <Value>(step: Value) => ({ ...contract, scenarios: [{ ...contract.scenarios[0], steps: [step] }] });

describe("parity input boundaries", () => {
	test("rejects duplicate area registrations and missing web handlers", () => {
		expect(() => mergeActionMaps({ openResources: () => {} }, { openResources: () => {} })).toThrow("Duplicate parity action: openResources");
		expect(() => requireRegisteredActions(["openResources"])).toThrow("typeSettingsField");
	});
	test("registration keeps an own action even when it is non-enumerable", () => {
		const openResources = () => {};
		const area = Object.defineProperty({ openResources }, "openResources", { enumerable: false });
		expect(mergeActionMaps(area).openResources).toBe(openResources);
	});
	test("overview identities retain an API version and reject malformed discriminators", () => {
		const step = { action: "openOverviewProblem", kind: "Deployment", name: "same", namespace: "payments", context: "mock-dev", apiVersion: "alpha.example/v1" };
		expect(parseContract(withStep(step)).scenarios[0].steps[0]).toMatchObject({ apiVersion: "alpha.example/v1" });
		expect(() => parseContract(withStep({ ...step, apiVersion: 42 }))).toThrow();
	});
	test("rejects malformed contracts, IDs and steps", () => {
		for (const value of [null, {}, { ...contract, scenarios: [] }, { ...contract, scenarios: [{ ...contract.scenarios[0], id: "" }] }, { ...contract, scenarios: [...contract.scenarios, ...contract.scenarios] }, withStep({ action: "unknown" }), withStep({ action: "constructor" }), withStep({ action: "typeSettingsField", field: "Server URL", value: "fixture" }), withStep({ action: "typeSettingsField", field: "Server token", value: "" })]) {
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

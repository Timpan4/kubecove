import { describe, expect, test } from "bun:test";
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

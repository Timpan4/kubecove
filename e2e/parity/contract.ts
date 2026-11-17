import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import type { JsonValue } from "../../src/lib/types";

import { coverageSteps } from "./contract-coverage";
import { deliverySteps } from "./contract-delivery";
import { governanceSteps } from "./contract-governance";
import { integrationsSteps } from "./contract-integrations";
import { overlayMotionSteps } from "./contract-overlay-motion";
import { overviewSteps } from "./contract-overview";
import { resourcesSteps } from "./contract-resources";
import { sessionsSteps } from "./contract-sessions";
import { settingsSteps } from "./contract-settings";
import { yamlSteps } from "./contract-yaml";
import { isString, object, string } from "./contract-values";

const parsers = {
	...coverageSteps,
	...deliverySteps,
	...governanceSteps,
	...integrationsSteps,
	...overlayMotionSteps,
	...overviewSteps,
	...resourcesSteps,
	...sessionsSteps,
	...settingsSteps,
	...yamlSteps,
};

export type Action = keyof typeof parsers;
export type Step<ActionName extends Action = Action> = {
	[Name in ActionName]: { action: Name } & ReturnType<(typeof parsers)[Name]>;
}[ActionName];
export type Scenario = { id: string; title: string; steps: Step[] };
export type Contract = { scenarios: Scenario[] };

function isAction(value: JsonValue | undefined): value is Action {
	return isString(value) && Object.hasOwn(parsers, value);
}

export function parseStep<Value>(value: Value): Step {
	const item = object(value);
	const { action } = item;
	if (!isAction(action)) throw new Error(`Unknown action: ${String(action)}`);
	// SAFETY: The checked key selects both the payload parser and the Step discriminant.
	return { action, ...parsers[action](item) } as Step;
}

export function requireActionCoverage(contract: Contract): void {
	const exercised = new Set(contract.scenarios.flatMap((scenario) => scenario.steps.map((step) => step.action)));
	const missing = Object.keys(parsers).filter((action) => isAction(action) && !exercised.has(action));
	if (missing.length) throw new Error(`Missing parity scenarios for actions: ${missing.join(", ")}`);
}

export function parseContract<Value>(value: Value): Contract {
	const input = object(value);
	if (input.fixture !== "browser-mocks" || input.context !== "mock-dev" || !Array.isArray(input.scenarios) || !input.scenarios.length) throw new Error("Invalid parity contract");
	const ids = new Set<string>();
	return { scenarios: input.scenarios.map((value) => {
		const scenario = object(value);
		const id = string(scenario.id);
		if (ids.has(id)) throw new Error(`Duplicate scenario: ${id}`);
		ids.add(id);
		if (!Array.isArray(scenario.steps) || !scenario.steps.length) throw new Error(`Missing steps: ${id}`);
		return { id, title: string(scenario.title), steps: scenario.steps.map(parseStep) };
	}) };
}

export const SCENARIO_DIR = "e2e/parity/scenarios";

/** Reads every `*.json` scenario file in name order, as the GPUI driver does. IDs must be unique across files. */
export async function loadContract(directory = SCENARIO_DIR): Promise<Contract> {
	const files = (await readdir(directory)).filter((name) => name.endsWith(".json")).sort();
	if (!files.length) throw new Error(`No scenario files in ${directory}`);
	const scenarios: Scenario[] = [];
	const ids = new Set<string>();
	for (const file of files) {
		for (const scenario of parseContract(JSON.parse(await readFile(join(directory, file), "utf8"))).scenarios) {
			if (ids.has(scenario.id)) throw new Error(`Duplicate scenario: ${scenario.id} in ${file}`);
			ids.add(scenario.id);
			scenarios.push(scenario);
		}
	}
	return { scenarios };
}

/** The equal-tests rule: every view the GPUI client renders needs a scenario whose ID starts with `<view>.`. */
export function requireViewCoverage(contract: Contract, views: readonly string[]): void {
	const missing = views.filter((view) => !contract.scenarios.some((scenario) => scenario.id.startsWith(`${view}.`)));
	if (missing.length) throw new Error(`Views without shared parity scenarios: ${missing.join(", ")}`);
}

export function validateReports(contract: Contract, reports: unknown[]): void {
	const drivers = new Set<string>();
	for (const value of reports) {
		const report = object(value);
		const driver = string(report.driver);
		if ((driver !== "web" && driver !== "gpui") || drivers.has(driver)) throw new Error(`Unexpected or duplicate driver: ${driver}`);
		drivers.add(driver);
		if (!Array.isArray(report.scenarios)) throw new Error(`Missing scenarios: ${driver}`);
		const expected = new Set(contract.scenarios.map((scenario) => scenario.id));
		for (const value of report.scenarios) {
			const result = object(value);
			const id = string(result.id);
			if (!expected.delete(id) || result.status !== "passed") throw new Error(`Invalid ${driver} result: ${id}`);
		}
		if (expected.size) throw new Error(`Missing ${driver} results: ${[...expected].join(", ")}`);
	}
	if (drivers.size !== 2) throw new Error("Missing driver report");
}

type Identity = { kind: string; name: string; namespace: string };
export type Step =
	| { action: "openResources" | "showYaml" | "closeDetail" | "expectDetailClosed" | "expectEmpty" | "refresh" }
	| { action: "search" | "expectSearch"; value: string }
	| ({ action: "expectResource" | "openResource" | "expectDetail" } & Identity)
	| { action: "expectYaml"; contains: string[] };
export type Contract = { scenarios: { id: string; title: string; steps: Step[] }[] };

function object(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected object");
	return Object.fromEntries(Object.entries(value));
}
function string(value: unknown): string {
	if (typeof value !== "string" || !value.trim()) throw new Error("Expected nonempty string");
	return value;
}
function step(value: unknown): Step {
	const item = object(value);
	switch (item.action) {
		case "openResources": case "showYaml": case "closeDetail": case "expectDetailClosed": case "expectEmpty": case "refresh":
			return { action: item.action };
		case "search": case "expectSearch": return { action: item.action, value: string(item.value) };
		case "expectResource": case "openResource": case "expectDetail": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "expectYaml":
			if (!Array.isArray(item.contains) || !item.contains.length) throw new Error("Expected YAML fragments");
			return { action: item.action, contains: item.contains.map(string) };
		default: throw new Error(`Unknown action: ${String(item.action)}`);
	}
}
export function parseContract(value: unknown): Contract {
	const input = object(value);
	if (input.fixture !== "browser-mocks" || input.context !== "mock-dev" || !Array.isArray(input.scenarios) || !input.scenarios.length) throw new Error("Invalid parity contract");
	const ids = new Set<string>();
	return { scenarios: input.scenarios.map((value: unknown) => {
		const scenario = object(value);
		const id = string(scenario.id);
		if (ids.has(id)) throw new Error(`Duplicate scenario: ${id}`);
		ids.add(id);
		if (!Array.isArray(scenario.steps) || !scenario.steps.length) throw new Error(`Missing steps: ${id}`);
		return { id, title: string(scenario.title), steps: scenario.steps.map(step) };
	}) };
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

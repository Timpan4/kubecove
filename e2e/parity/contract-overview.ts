import type { JsonObject } from "../../src/lib/types";
import { oneOf, string } from "./contract-values";
const FILTERS = ["Degraded", "Needs attention", "Healthy", "Unknown", "Not evaluated", "Restart evidence"] as const;
type Identity = { kind: string; name: string; namespace: string; context: string };
export type OverviewStep =
	| ({ action: "expectOverviewProblem"; ready: string; status: string } & Identity)
	| ({ action: "openOverviewProblem" } & Identity)
	| ({ action: "toggleOverviewProblem" } & Identity)
	| ({ action: "expectNoOverviewProblem" } & Identity)
	| { action: "overviewHealth"; filter: typeof FILTERS[number] }
	| { action: "overviewNamespace"; namespace: string };
export function overviewStep(item: JsonObject): OverviewStep | undefined {
	switch (item.action) {
		case "overviewHealth": return { action: item.action, filter: oneOf(FILTERS, item.filter, "a health filter") };
		case "overviewNamespace": return { action: item.action, namespace: string(item.namespace) };
		case "expectOverviewProblem": case "openOverviewProblem": case "toggleOverviewProblem": case "expectNoOverviewProblem": {
			const identity = { kind: string(item.kind), name: string(item.name), namespace: string(item.namespace), context: item.context === undefined ? "mock-dev" : string(item.context) };
			return item.action !== "expectOverviewProblem" ? { action: item.action, ...identity } :
				{ action: item.action, ...identity, ready: string(item.ready), status: string(item.status) };
		}
		default: return undefined;
	}
}

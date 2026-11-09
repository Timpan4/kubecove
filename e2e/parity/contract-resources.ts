import type { Step } from "./contract-steps";
import { oneOf, string } from "./contract-values";

/** Parses resource table keyboard steps; returns undefined for any other action. */
export function resourceTableStep(item: Record<string, unknown>): Step | undefined {
	switch (item.action) {
		case "pressResourceRow": return { action: item.action, key: oneOf(["Enter", "Space"], item.key, "Enter or Space"), kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		default: return undefined;
	}
}

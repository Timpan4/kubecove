import type { Step } from "./contract-steps";
import { boolean, oneOf, string, strings } from "./contract-values";

/** Parses resource table, multi-context and map context steps; returns undefined for any other action. */
export function resourceTableStep(item: Record<string, unknown>): Step | undefined {
	switch (item.action) {
		case "pressResourceRow": return { action: item.action, key: oneOf(["Enter", "Space"], item.key, "Enter or Space"), kind: string(item.kind), name: string(item.name), namespace: string(item.namespace) };
		case "openResource": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace), ...(item.context === undefined ? {} : { context: string(item.context) }) };
		case "expectRowContexts": return { action: item.action, kind: string(item.kind), name: string(item.name), namespace: string(item.namespace), contexts: strings(item.contexts, false) };
		case "filterContexts": return { action: item.action, contexts: strings(item.contexts, false) };
		case "expectResourceSummary": return { action: item.action, text: string(item.text) };
		case "selectMapContext": return { action: item.action, context: string(item.context) };
		case "expectMapContext":
			if (!boolean(item.shown)) {
				if (item.context !== undefined) throw new Error("A hidden map context select has no value");
				return { action: item.action, shown: false };
			}
			return { action: item.action, shown: true, context: string(item.context) };
		default: return undefined;
	}
}

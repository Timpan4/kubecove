import { parse } from "yaml";
import type { JsonObject, JsonValue } from "@/lib/types";

/** The Application `spec`, or null when the YAML is missing or unreadable. */
export function applicationSpecFromYaml(yaml: string | undefined): JsonObject | null {
	if (!yaml) return null;
	try {
		const value: JsonValue = parse(yaml);
		return isRecord(value) && isRecord(value.spec) ? value.spec : null;
	} catch {
		return null;
	}
}

/**
 * Argo CD runs automated sync whenever `syncPolicy.automated` is set, unless
 * `automated.enabled` is false. `prune` and `selfHeal` only qualify it.
 */
export function argoSyncPolicyLabel(spec: JsonObject | null): string {
	if (!spec) return "Policy unavailable";
	const automated = isRecord(spec.syncPolicy) ? spec.syncPolicy.automated : undefined;
	if (!isRecord(automated) || automated.enabled === false) return "Manual";
	const parts = ["Automated"];
	if (automated.prune === true) parts.push("prune");
	if (automated.selfHeal === true) parts.push("self-heal");
	return parts.join(" · ");
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

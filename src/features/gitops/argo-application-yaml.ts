import { parse } from "yaml";
import type { JsonObject, JsonValue } from "@/lib/types";

export function applicationSpecFromYaml(yaml: string | undefined): JsonObject {
	if (!yaml) return {};
	try {
		const value: JsonValue = parse(yaml);
		return isRecord(value) && isRecord(value.spec) ? value.spec : {};
	} catch {
		return {};
	}
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

import type { ArgoApplicationHistory, JsonObject, JsonValue } from "@/lib/types";

export function argoHistoryRevision(entry: ArgoApplicationHistory): string {
	return entry.revision ?? (entry.revisions.join(", ") || "Unknown revision");
}

export function argoStatusValue(value: JsonValue | undefined, ...path: string[]): string | null {
	let current: JsonValue | undefined = value;
	for (const key of path) {
		if (!isRecord(current)) return null;
		current = current[key];
	}
	return isString(current) && current.trim() ? current : null;
}

function valueAt(value: JsonValue | undefined, ...path: string[]): JsonValue | undefined {
	let current = value;
	for (const key of path) {
		if (Array.isArray(current)) {
			const index = Number(key);
			if (!Number.isInteger(index)) return undefined;
			current = current[index];
			continue;
		}
		if (!isRecord(current)) return undefined;
		current = current[key];
	}
	return current;
}

export function argoStringAt(value: JsonValue | undefined, ...path: string[]): string | null {
	const current = valueAt(value, ...path);
	return isString(current) && current.trim() ? current : null;
}

export function argoBooleanAt(value: JsonValue | undefined, ...path: string[]): boolean | null {
	const current = valueAt(value, ...path);
	return current === true || current === false ? current : null;
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function isString(value: JsonValue | undefined): value is string {
	return String(value) === value;
}

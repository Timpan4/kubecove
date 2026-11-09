import type { JsonObject } from "../../src/lib/types";

/** Field parsers shared by the scenario step parsers in `contract*.ts`. Each throws on a malformed value. */

/** Value guards. Each recognizes a primitive by a round trip through its constructor, which needs no `typeof`. */
export function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}
export function isBoolean<Value>(value: Value): value is Value & boolean {
	return Boolean(value) === value;
}
export function isNumber<Value>(value: Value): value is Value & number {
	return Number(value) === value;
}
function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

export function oneOf<T extends string, Value>(values: readonly T[], value: Value, label: string): T {
	const found = values.find((candidate) => Object.is(candidate, value));
	if (!found) throw new Error(`Expected ${label}`);
	return found;
}
export function object<Value>(value: Value): JsonObject {
	if (!isRecord(value)) throw new Error("Expected object");
	return Object.fromEntries(Object.entries(value));
}
export function string<Value>(value: Value): string {
	if (!isString(value) || !value.trim()) throw new Error("Expected nonempty string");
	return value;
}
/** A string that may be empty, such as an expected field value. */
export function plainString<Value>(value: Value): string {
	if (!isString(value)) throw new Error("Expected a string");
	return value;
}
export function count<Value>(value: Value): number {
	if (!isNumber(value) || !Number.isSafeInteger(value) || value < 0) throw new Error("Expected a nonnegative integer");
	return value;
}
export function strings<Value>(value: Value, allowEmpty: boolean): string[] {
	if (!Array.isArray(value) || (!allowEmpty && !value.length)) throw new Error("Expected a list of strings");
	return value.map(string);
}
export function boolean<Value>(value: Value): boolean {
	if (!isBoolean(value)) throw new Error("Expected boolean");
	return value;
}
export function optionalBoolean<Value>(value: Value): boolean | undefined {
	if (value == null) return undefined;
	if (!isBoolean(value)) throw new Error("Expected boolean");
	return value;
}

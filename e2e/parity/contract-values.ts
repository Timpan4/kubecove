/** Field parsers shared by the scenario step parsers in `contract*.ts`. Each throws on a malformed value. */
export function oneOf<T extends string>(values: readonly T[], value: unknown, label: string): T {
	const found = values.find((candidate) => candidate === value);
	if (!found) throw new Error(`Expected ${label}`);
	return found;
}
export function object(value: unknown): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected object");
	return Object.fromEntries(Object.entries(value));
}
export function string(value: unknown): string {
	if (typeof value !== "string" || !value.trim()) throw new Error("Expected nonempty string");
	return value;
}
/** A string that may be empty, such as an expected field value. */
export function plainString(value: unknown): string {
	if (typeof value !== "string") throw new Error("Expected a string");
	return value;
}
export function count(value: unknown): number {
	if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new Error("Expected a nonnegative integer");
	return value;
}
export function strings(value: unknown, allowEmpty: boolean): string[] {
	if (!Array.isArray(value) || (!allowEmpty && !value.length)) throw new Error("Expected a list of strings");
	return value.map(string);
}
export function boolean(value: unknown): boolean {
	if (typeof value !== "boolean") throw new Error("Expected boolean");
	return value;
}
export function optionalBoolean(value: unknown): boolean | undefined {
	if (value == null) return undefined;
	if (typeof value !== "boolean") throw new Error("Expected boolean");
	return value;
}

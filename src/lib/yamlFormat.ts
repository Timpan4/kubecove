import { parse, parseDocument, stringify, type YAMLError } from "yaml";
import type { JsonObject, JsonValue, YamlEncoding } from "@/lib/types";

const YAML_PARSE_OPTIONS = {
	prettyErrors: false,
	strict: true,
	uniqueKeys: true,
} as const;

const YAML_FORMAT_OPTIONS = {
	indent: 2,
	lineWidth: 0,
} as const;

type FormattableYamlValue = JsonValue | undefined;

export function formatYamlDocument(
	value: string,
	encoding: YamlEncoding = "yaml",
): string {
	const document = parseDocument(value, YAML_PARSE_OPTIONS);
	if (document.errors.length > 0) {
		throw new Error(document.errors[0]?.message ?? "YAML parse failed.");
	}
	if (encoding === "kyaml") {
		const parsed: JsonValue = parse(value);
		return `${formatKyamlValue(parsed, 0)}\n`;
	}
	return stringify(parse(value), YAML_FORMAT_OPTIONS);
}

function isBoolean(value: FormattableYamlValue): value is boolean {
	return Boolean(value) === value;
}

function isNumber(value: FormattableYamlValue): value is number {
	return Object.is(Number(value), value);
}

function isString(value: FormattableYamlValue): value is string {
	return String(value) === value;
}

function isJsonObject(value: FormattableYamlValue): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function formatKyamlValue(value: FormattableYamlValue, indent: number): string {
	if (value === null || value === undefined) return "null";
	if (isBoolean(value) || isNumber(value)) return String(value);
	if (isString(value)) return JSON.stringify(value);
	if (Array.isArray(value)) return formatKyamlArray(value, indent);
	if (isJsonObject(value)) {
		return formatKyamlObject(value, indent);
	}
	return JSON.stringify(String(value));
}

function formatKyamlArray(values: JsonValue[], indent: number): string {
	if (values.length === 0) return "[]";
	const childIndent = indent + 2;
	const lines = values.map(
		(value) => `${" ".repeat(childIndent)}${formatKyamlValue(value, childIndent)},`,
	);
	return `[\n${lines.join("\n")}\n${" ".repeat(indent)}]`;
}

function formatKyamlObject(value: JsonObject, indent: number): string {
	const entries = Object.entries(value);
	if (entries.length === 0) return "{}";
	const childIndent = indent + 2;
	const lines = entries.map(
		([key, child]) =>
			`${" ".repeat(childIndent)}${formatKyamlKey(key)}: ${formatKyamlValue(child, childIndent)},`,
	);
	return `{\n${lines.join("\n")}\n${" ".repeat(indent)}}`;
}

function formatKyamlKey(key: string): string {
	return /^[A-Za-z_][A-Za-z0-9_./-]*$/.test(key)
		? key
		: JSON.stringify(key);
}

export interface YamlParseDiagnostics {
	errors: YAMLError[];
	warnings: YAMLError[];
}

export function parseYamlErrors(value: string): YamlParseDiagnostics {
	const document = parseDocument(value, YAML_PARSE_OPTIONS);
	return {
		errors: [...document.errors],
		warnings: [...document.warnings],
	};
}

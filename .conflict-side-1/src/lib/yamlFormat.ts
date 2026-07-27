import { parse, parseDocument, stringify, type YAMLError } from "yaml";
import type { YamlEncoding } from "@/lib/types";

const YAML_PARSE_OPTIONS = {
	prettyErrors: false,
	strict: true,
	uniqueKeys: true,
} as const;

const YAML_FORMAT_OPTIONS = {
	indent: 2,
	lineWidth: 0,
} as const;

export function formatYamlDocument(
	value: string,
	encoding: YamlEncoding = "yaml",
): string {
	const document = parseDocument(value, YAML_PARSE_OPTIONS);
	if (document.errors.length > 0) {
		throw new Error(document.errors[0]?.message ?? "YAML parse failed.");
	}
	if (encoding === "kyaml") {
		return `${formatKyamlValue(parse(value), 0)}\n`;
	}
	return stringify(parse(value), YAML_FORMAT_OPTIONS);
}

function formatKyamlValue(value: unknown, indent: number): string {
	if (value === null || value === undefined) return "null";
	if (typeof value === "boolean" || typeof value === "number") return String(value);
	if (typeof value === "string") return JSON.stringify(value);
	if (Array.isArray(value)) return formatKyamlArray(value, indent);
	if (typeof value === "object") {
		return formatKyamlObject(value as Record<string, unknown>, indent);
	}
	return JSON.stringify(String(value));
}

function formatKyamlArray(values: unknown[], indent: number): string {
	if (values.length === 0) return "[]";
	const childIndent = indent + 2;
	const lines = values.map(
		(value) => `${" ".repeat(childIndent)}${formatKyamlValue(value, childIndent)},`,
	);
	return `[\n${lines.join("\n")}\n${" ".repeat(indent)}]`;
}

function formatKyamlObject(value: Record<string, unknown>, indent: number): string {
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

export function parseYamlErrors(
	value: string,
): { errors: YAMLError[]; warnings: YAMLError[] } {
	const document = parseDocument(value, YAML_PARSE_OPTIONS);
	return {
		errors: [...document.errors],
		warnings: [...document.warnings],
	};
}

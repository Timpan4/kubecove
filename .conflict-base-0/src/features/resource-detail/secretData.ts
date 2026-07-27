import { parseDocument } from "yaml";

export type SecretDataEntry = {
	id: string;
	key: string;
	value: string;
	source: "data" | "stringData";
	masked: boolean;
};

export type SecretDataDecodeResult =
	| { value: string; error?: never }
	| { value?: never; error: "invalid-base64" | "binary-data" };

export function parseSecretData(yamlText: string): SecretDataEntry[] {
	const document = parseDocument(yamlText, { prettyErrors: false, strict: true });
	if (document.errors.length > 0 || !isRecord(document.toJSON())) return [];
	const secret = document.toJSON() as Record<string, unknown>;
	return [
		...entriesFrom(secret.data, "data"),
		...entriesFrom(secret.stringData, "stringData"),
	];
}

export function isMaskedSecretValue(value: string): boolean {
	const normalized = value.trim().toLowerCase();
	return (
		normalized === "redacted" ||
		normalized === "[redacted]" ||
		normalized === "<redacted>" ||
		/^\*+$/.test(normalized)
	);
}

export function maskedSecretValue(): string {
	return "••••••";
}

export function decodeSecretDataValue(value: string): SecretDataDecodeResult {
	if (!isValidBase64(value)) return { error: "invalid-base64" };
	try {
		const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
		return { value: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
	} catch {
		return { error: "binary-data" };
	}
}

function entriesFrom(value: unknown, source: SecretDataEntry["source"]): SecretDataEntry[] {
	if (!isRecord(value)) return [];
	return Object.entries(value).map(([key, entryValue]) => {
		const entry = String(entryValue);
		return {
			id: `${source}:${key}`,
			key,
			value: entry,
			source,
			masked: isMaskedSecretValue(entry),
		};
	});
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isValidBase64(value: string): boolean {
	return value.length % 4 === 0 && /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value);
}

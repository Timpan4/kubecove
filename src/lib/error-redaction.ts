const SENSITIVE_DETAIL_PATTERNS: Array<[RegExp, string]> = [
	[/(\bauthorization\s*:\s*(?:basic|bearer)\s+)[^\s,;]+/gi, "$1[REDACTED]"],
	[/(\b(?:x-api-key|api-key)\s*:\s*)[^\s,;]+/gi, "$1[REDACTED]"],
	[/(\bbearer\s+)[A-Za-z0-9._~+/-]+=*/gi, "$1[REDACTED]"],
	[/(https?:\/\/)[^/\s:@]+:[^/\s@]+@/gi, "$1[REDACTED]@"],
	[
		/([?&](?:access[_-]?token|id[_-]?token|token|password|client[_-]?secret|x-amz-credential|x-amz-security-token|x-amz-signature|sig|signature|(?:x[-_])?api[-_]?key)=)[^&#\s]+/gi,
		"$1[REDACTED]",
	],
	[
		/(["']?(?:access[_-]?token|id[_-]?token|token|password|client[_-]?secret|(?:x[-_])?api[-_]?key)["']?\s*[:=]\s*["']?)[^"',\s}]+/gi,
		"$1[REDACTED]",
	],
];

export function redactSensitiveErrorDetail(detail: string): string {
	return SENSITIVE_DETAIL_PATTERNS.reduce(
		(redacted, [pattern, replacement]) => redacted.replace(pattern, replacement),
		detail,
	);
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && Object(value) === value;
}

function isString<Value>(value: Value): value is Value & string {
	// oxlint-disable-next-line anti-slop/no-runtime-typeof -- Error details may be untrusted JSON without a callable toString.
	return typeof value === "string";
}

export function messageFromError(cause: unknown): string {
	const detail =
		cause instanceof Error
			? cause.message
			: isString(cause)
				? cause
				: isRecord(cause) && isString(cause.message)
					? cause.message
					: "Unknown error";
	return redactSensitiveErrorDetail(detail);
}
import type { JsonObject } from "./types";

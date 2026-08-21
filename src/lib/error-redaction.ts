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

export function messageFromError(error: unknown): string {
	const detail =
		error instanceof Error
			? error.message
			: typeof error === "string"
				? error
				: typeof error === "object" &&
					  error !== null &&
					  "message" in error &&
					  typeof error.message === "string"
					? error.message
					: "Unknown error";
	return redactSensitiveErrorDetail(detail);
}

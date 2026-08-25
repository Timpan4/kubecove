const DETERMINISTIC_KINDS = new Set([
	"admissionDenied",
	"fieldManagerConflict",
	"forbidden",
	"immutableField",
	"invalidResource",
	"kubeconfig",
	"kubeconfigConfig",
	"liveSessionTargetUnavailable",
	"notFound",
	"providerDiscoveryUnavailable",
	"providerUnavailable",
	"validation",
	"serialization",
]);

const DETERMINISTIC_MESSAGE =
	/\bforbidden\b|\b403\b|\bnot ?found\b|\b404\b|admission webhook|fieldmanagerconflict|field is immutable|pod updates may not change fields|kubeconfig|failed to infer config/i;

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && Object(value) === value;
}

function messageFromError(cause: unknown): string {
	if (cause instanceof Error) return cause.message;
	if (isRecord(cause)) {
		return String(cause.message ?? "");
	}
	return String(cause);
}

export function isTransientQueryError(cause: unknown): boolean {
	if (isRecord(cause)) {
		const kind = cause.kind;
		if (String(kind) === kind && DETERMINISTIC_KINDS.has(kind)) {
			return false;
		}
	}

	// Backend kind is coarse: "cluster" covers both transient network errors and deterministic 403/404s.
	return !DETERMINISTIC_MESSAGE.test(messageFromError(cause));
}

export function queryRetry(failureCount: number, cause: unknown): boolean {
	return isTransientQueryError(cause) && failureCount < 2;
}
import type { JsonObject } from "./types";

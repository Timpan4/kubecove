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

function messageFromError(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (
		typeof error === "object" &&
		error !== null &&
		"message" in error
	) {
		return String((error as { message?: unknown }).message ?? "");
	}
	return String(error);
}

export function isTransientQueryError(error: unknown): boolean {
	if (typeof error === "object" && error !== null && "kind" in error) {
		const kind = (error as { kind?: unknown }).kind;
		if (typeof kind === "string" && DETERMINISTIC_KINDS.has(kind)) {
			return false;
		}
	}

	// Backend kind is coarse: "cluster" covers both transient network errors and deterministic 403/404s.
	return !DETERMINISTIC_MESSAGE.test(messageFromError(error));
}

export function queryRetry(failureCount: number, error: unknown): boolean {
	return isTransientQueryError(error) && failureCount < 2;
}

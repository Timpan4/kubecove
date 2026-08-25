import type { ArgoOperationConfirmation, ArgoOperationRequest } from "@/lib/gitops-types";
import type { JsonObject, JsonValue } from "@/lib/types";

interface Preflight {
	allowed?: JsonValue;
	sessionId?: JsonValue;
	expiresAt?: JsonValue;
	reviewedRequest?: ArgoOperationRequest | JsonValue;
	reason?: JsonValue;
}

interface Result {
	accepted?: JsonValue;
	message?: JsonValue;
}

export class ArgoOperationRefreshError extends Error {
	constructor(cause: unknown) {
		const detail = cause instanceof Error ? cause.message : isString(cause) ? cause : "";
		super(
			`Operation accepted, but latest Application state could not be loaded${detail ? `: ${detail}` : "."}`,
		);
		this.name = "ArgoOperationRefreshError";
	}
}

export type ArgoOperationPhase =
	| "authorizing"
	| "submitting"
	| "refreshing"
	| "accepted"
	| "error";

export async function runArgoOperationLifecycle({
	request,
	preflight,
	run,
	refresh,
	isCurrent = () => true,
	onPhase,
}: {
	request: ArgoOperationRequest;
	preflight: (request: ArgoOperationRequest) => Promise<Preflight>;
	run: (confirmation: ArgoOperationConfirmation) => Promise<Result>;
	refresh: () => Promise<void>;
	isCurrent?: () => boolean;
	onPhase?: (phase: ArgoOperationPhase, cause?: unknown) => void;
}): Promise<void> {
	const phase = (next: ArgoOperationPhase, cause?: unknown) => {
		if (isCurrent()) onPhase?.(next, cause);
	};
	try {
		phase("authorizing");
		const result = await preflight(request);
		if (
			result?.allowed !== true ||
			!isString(result.sessionId) ||
			!result.sessionId ||
			!isNumber(result.expiresAt) ||
			!isRequest(result.reviewedRequest)
		) {
			throw new Error(isString(result?.reason) ? result.reason : "Operation unavailable");
		}
		phase("submitting");
		const operation = await run({ sessionId: result.sessionId, confirmation: result.sessionId });
		if (operation?.accepted !== true) {
			throw new Error(isString(operation?.message) ? operation.message : "Operation rejected");
		}
		phase("refreshing");
		try {
			await refresh();
		} catch (error) {
			throw new ArgoOperationRefreshError(error);
		}
		phase("accepted");
	} catch (error) {
		phase("error", error);
		throw error;
	}
}

function isRequest(
	value: ArgoOperationRequest | JsonValue | undefined,
): value is ArgoOperationRequest {
	if (!isRecord(value)) return false;
	const request = value;
	return (
		(request.transport === "connected" || request.transport === "kubernetes") &&
		isRecord(request.application) &&
		isString(request.application.name) &&
		Array.isArray(request.resources)
	);
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}

function isNumber<Value>(value: Value): value is Value & number {
	return Number(value) === value;
}

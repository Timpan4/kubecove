import type { ArgoOperationConfirmation, ArgoOperationRequest } from "@/lib/gitops-types";

interface Preflight {
	allowed?: unknown;
	sessionId?: unknown;
	expiresAt?: unknown;
	reviewedRequest?: unknown;
	reason?: unknown;
}

interface Result {
	accepted?: unknown;
	message?: unknown;
}

export class ArgoOperationRefreshError extends Error {
	constructor(error: unknown) {
		const detail = error instanceof Error ? error.message : typeof error === "string" ? error : "";
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
	refresh: () => Promise<unknown>;
	isCurrent?: () => boolean;
	onPhase?: (phase: ArgoOperationPhase, error?: unknown) => void;
}): Promise<void> {
	const phase = (next: ArgoOperationPhase, error?: unknown) => {
		if (isCurrent()) onPhase?.(next, error);
	};
	try {
		phase("authorizing");
		const result = await preflight(request);
		if (
			result?.allowed !== true ||
			typeof result.sessionId !== "string" ||
			!result.sessionId ||
			typeof result.expiresAt !== "number" ||
			!isRequest(result.reviewedRequest)
		) {
			throw new Error(typeof result?.reason === "string" ? result.reason : "Operation unavailable");
		}
		phase("submitting");
		const operation = await run({ sessionId: result.sessionId, confirmation: result.sessionId });
		if (operation?.accepted !== true) {
			throw new Error(typeof operation?.message === "string" ? operation.message : "Operation rejected");
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

function isRequest(value: unknown): value is ArgoOperationRequest {
	if (typeof value !== "object" || value === null) return false;
	const request = value as Partial<ArgoOperationRequest>;
	return (
		(request.transport === "connected" || request.transport === "kubernetes") &&
		typeof request.application === "object" &&
		request.application !== null &&
		typeof request.application.name === "string" &&
		Array.isArray(request.resources)
	);
}

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

export async function runArgoOperationLifecycle({
	request,
	preflight,
	run,
	refresh,
}: {
	request: ArgoOperationRequest;
	preflight: (request: ArgoOperationRequest) => Promise<Preflight>;
	run: (confirmation: ArgoOperationConfirmation) => Promise<Result>;
	refresh: () => Promise<unknown>;
}): Promise<void> {
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
	const operation = await run({ sessionId: result.sessionId, confirmation: result.sessionId });
	if (operation?.accepted !== true) {
		throw new Error(typeof operation?.message === "string" ? operation.message : "Operation rejected");
	}
	try {
		await refresh();
	} catch (error) {
		throw new ArgoOperationRefreshError(error);
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

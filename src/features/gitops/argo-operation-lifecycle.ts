import type { ArgoOperationRequest } from "@/lib/gitops-types";

interface Preflight {
	allowed?: unknown;
	preflightToken?: unknown;
	resolvedRequest?: unknown;
	reason?: unknown;
}

interface Result {
	accepted?: unknown;
	message?: unknown;
}

export async function runArgoOperationLifecycle({
	request,
	preflight,
	run,
	refresh,
}: {
	request: ArgoOperationRequest;
	preflight: (request: ArgoOperationRequest) => Promise<Preflight>;
	run: (request: ArgoOperationRequest) => Promise<Result>;
	refresh: () => Promise<unknown>;
}): Promise<void> {
	const result = await preflight(request);
	if (
		result?.allowed !== true ||
		typeof result.preflightToken !== "string" ||
		!result.preflightToken ||
		!isRequest(result.resolvedRequest)
	) {
		throw new Error(typeof result?.reason === "string" ? result.reason : "Operation unavailable");
	}
	const operation = await run({ ...result.resolvedRequest, preflightToken: result.preflightToken });
	if (operation?.accepted !== true) {
		throw new Error(typeof operation?.message === "string" ? operation.message : "Operation rejected");
	}
	await refresh();
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

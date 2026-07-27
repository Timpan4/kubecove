import type { CancellableRequest } from "./types";

let requestSequence = 0;

export function createCancelScope(label: string, parts: unknown): string {
	return `${label}:${JSON.stringify(parts)}`;
}

export function createCancellableRequest(
	cancelScope: string,
	label: string,
): CancellableRequest {
	requestSequence = (requestSequence + 1) % Number.MAX_SAFE_INTEGER;
	return {
		cancelScope,
		requestId: `${label}-${Date.now().toString(36)}-${requestSequence.toString(36)}`,
	};
}

export function cancellableArg(
	request?: CancellableRequest,
): Partial<CancellableRequest> {
	if (!request) return {};
	return request;
}

import type { CancellableRequest } from "./types";

export function kubeconfigArg(kubeconfigEnvVar?: string): {
	kubeconfigEnvVar?: string;
} {
	if (
		kubeconfigEnvVar === undefined ||
		kubeconfigEnvVar.startsWith("kubeconfigSource=")
	) {
		return {};
	}
	return { kubeconfigEnvVar };
}

export function cancellableArg(
	request?: CancellableRequest,
): Partial<CancellableRequest> {
	if (!request) return {};
	return request;
}

export function sanitizeKubeconfigRequest<T extends { kubeconfigEnvVar?: string }>(
	request: T,
): T {
	if (!request.kubeconfigEnvVar?.startsWith("kubeconfigSource=")) return request;
	const { kubeconfigEnvVar: _ignored, ...rest } = request;
	return rest as T;
}

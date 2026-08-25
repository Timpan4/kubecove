import type { CancellableRequest } from "./types";

interface KubeconfigArg {
	kubeconfigEnvVar?: string;
}

export function kubeconfigArg(kubeconfigEnvVar?: string): KubeconfigArg {
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
	const sanitized = { ...request };
	delete sanitized.kubeconfigEnvVar;
	return sanitized;
}

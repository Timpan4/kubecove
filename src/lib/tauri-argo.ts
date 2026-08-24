import { cancellableArg } from "./finite-read-lifecycle";
import { kubeconfigArg } from "./tauri-args";
import type { TauriClient } from "./tauri-runtime";
import type {
	ArgoApplicationInspector,
	ArgoApplicationRef,
	ArgoConnectionProfile,
	ArgoConnectionStatus,
	ArgoManagedResource,
	ArgoOperationConfirmation,
	ArgoOperationPreflight,
	ArgoOperationRequest,
	ArgoOperationResult,
	ArgoResourceComparison,
	ArgoServerCapability,
	ArgoServerEndpoint,
	CancellableRequest,
} from "./types";

export async function discoverArgoServers(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<ArgoServerCapability[]> {
	return client.invoke<ArgoServerCapability[]>("discover_argo_servers", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
	});
}

export async function connectArgoServer(
	client: TauriClient,
	request: {
		id: string;
		serverUrl: string;
		endpoint: ArgoServerEndpoint;
		token?: string;
		username?: string;
		password?: string;
		insecureTls: boolean;
		customCaPem?: number[];
		rememberCredential: boolean;
		clusterContext?: string;
		kubeconfigEnvVar?: string;
		workspaceId?: string;
	},
): Promise<ArgoConnectionStatus> {
	const { kubeconfigEnvVar, ...args } = request;
	return client.invoke<ArgoConnectionStatus>("connect_argo_server", {
		...args,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export function getArgoConnectionStatus(
	client: TauriClient,
	id: string,
): Promise<ArgoConnectionStatus> {
	return client.invoke<ArgoConnectionStatus>("get_argo_connection_status", { id });
}

export function disconnectArgoServer(client: TauriClient, id: string): Promise<void> {
	return client.invoke<void>("disconnect_argo_server", { id });
}

export function forgetArgoCredential(
	client: TauriClient,
	profile: ArgoConnectionProfile,
): Promise<void> {
	return client.invoke<void>("forget_argo_credential", { profile });
}

export function getArgoApplicationInspector(
	client: TauriClient,
	request: {
		clusterContext: string;
		kubeconfigEnvVar?: string;
		connectionId?: string;
		transport: "connected" | "kubernetes";
		application: ArgoApplicationRef;
		redactSecrets?: boolean;
	},
	cancellable?: CancellableRequest,
): Promise<ArgoApplicationInspector> {
	return client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", {
		...request,
		...cancellableArg(cancellable),
	});
}

export function getArgoResourceComparison(
	client: TauriClient,
	request: {
		clusterContext: string;
		kubeconfigEnvVar?: string;
		connectionId?: string;
		transport: "connected" | "kubernetes";
		application: ArgoApplicationRef;
		resource: ArgoManagedResource;
		redactSecrets?: boolean;
	},
	cancellable?: CancellableRequest,
): Promise<ArgoResourceComparison> {
	return client.invoke<ArgoResourceComparison>("get_argo_resource_comparison", {
		...request,
		...cancellableArg(cancellable),
	});
}

export function preflightArgoOperation(
	client: TauriClient,
	request: ArgoOperationRequest,
): Promise<ArgoOperationPreflight> {
	return client.invoke<ArgoOperationPreflight>("preflight_argo_operation", { request });
}

export function runArgoOperation(
	client: TauriClient,
	confirmation: ArgoOperationConfirmation,
): Promise<ArgoOperationResult> {
	return client.invoke<ArgoOperationResult>("run_argo_operation", { confirmation });
}

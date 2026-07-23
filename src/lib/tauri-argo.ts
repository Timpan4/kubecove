import { kubeconfigArg } from "./tauri-args";
import type { TauriClient } from "./tauri-runtime";
import type {
	ArgoApplicationInspector,
	ArgoApplicationRef,
	ArgoConnectionProfile,
	ArgoConnectionStatus,
	ArgoManagedResource,
	ArgoOperationPreflight,
	ArgoOperationRequest,
	ArgoOperationResult,
	ArgoResourceComparison,
	ArgoServerCapability,
} from "./types";

export async function discoverArgoServers(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<ArgoServerCapability[]> {
	return client.invoke<ArgoServerCapability[]>("discover_argo_servers", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function connectArgoServer(
	client: TauriClient,
	request: {
		id: string;
		serverUrl: string;
		token?: string;
		username?: string;
		password?: string;
		insecureTls: boolean;
		customCaPem?: number[];
		rememberCredential: boolean;
		clusterContext?: string;
		workspaceId?: string;
	},
): Promise<ArgoConnectionStatus> {
	return client.invoke<ArgoConnectionStatus>("connect_argo_server", request);
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
): Promise<ArgoApplicationInspector> {
	return client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", request);
}

export function getArgoApplicationResources(
	client: TauriClient,
	request: {
		clusterContext: string;
		kubeconfigEnvVar?: string;
		connectionId?: string;
		transport: "connected" | "kubernetes";
		application: ArgoApplicationRef;
		redactSecrets?: boolean;
	},
): Promise<ArgoManagedResource[]> {
	return client.invoke<ArgoManagedResource[]>("get_argo_application_resources", request);
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
): Promise<ArgoResourceComparison> {
	return client.invoke<ArgoResourceComparison>("get_argo_resource_comparison", request);
}

export function preflightArgoOperation(
	client: TauriClient,
	request: ArgoOperationRequest,
): Promise<ArgoOperationPreflight> {
	return client.invoke<ArgoOperationPreflight>("preflight_argo_operation", { request });
}

export function runArgoOperation(
	client: TauriClient,
	request: ArgoOperationRequest,
): Promise<ArgoOperationResult> {
	return client.invoke<ArgoOperationResult>("run_argo_operation", { request });
}

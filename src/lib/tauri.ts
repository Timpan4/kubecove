import {
	Channel,
	invoke,
	type InvokeOptions,
} from "@tauri-apps/api/core";
import type {
	ClusterContext,
	NamespaceSummary,
	DiscoveredResourceKind,
	PodLogStreamRequest,
	PodExecSessionMessage,
	PodExecSessionRequest,
	PodExecSessionSummary,
	PodExecTerminalSize,
	PortForwardRequest,
	PortForwardSessionSummary,
	ResourceListRequest,
	ResourceSummary,
	ResourceEventSummary,
	ResourceDetailsFull,
	ResourceTopology,
	TopologyMode,
	StreamMessage,
	WatchResourceKey,
	AppError,
	AppUsageMetrics,
	ResourceMetricsSummary,
	ArgoApplicationSummary,
	ArgoApplicationDetails,
	ArgoApplicationSetSummary,
	ArgoAppProjectSummary,
	ArgoApplicationSetDetails,
	ArgoAppProjectDetails,
	HelmReleaseSummary,
	HelmReleaseDetails,
	RbacInspectionSummary,
} from "./types";
import { diagnosticLog, diagnosticResultSummary } from "./diagnostics";

export interface TauriClient {
	invoke<T>(
		cmd: string,
		args?: Record<string, unknown>,
		options?: InvokeOptions,
	): Promise<T>;
}

export function createTauriClient(): TauriClient {
	return {
		invoke: async <T>(
			cmd: string,
			args?: Record<string, unknown>,
			options?: InvokeOptions,
		): Promise<T> => {
			const started = performance.now();
			diagnosticLog("tauri.invoke.start", {
				cmd,
				args: args ? Object.keys(args).join(",") : "",
			});
			try {
				const result = await invoke<T>(cmd, args, options);
				diagnosticLog("tauri.invoke.done", {
					cmd,
					ms: Math.round(performance.now() - started),
					result: diagnosticResultSummary(result),
				});
				return result;
			} catch (error) {
				diagnosticLog("tauri.invoke.error", {
					cmd,
					ms: Math.round(performance.now() - started),
					error: error instanceof Error ? error.message : String(error),
				});
				console.error(`[k8s-manager:tauri] ${cmd} error`, error);
				throw error;
			}
		},
	};
}

export function createMockTauriClient(
	mockResponses: Record<string, unknown>,
): TauriClient {
	return {
		invoke: async <T>(cmd: string): Promise<T> => {
			if (cmd in mockResponses) {
				return mockResponses[cmd] as T;
			}
			throw new Error(`No mock response for command: ${cmd}`);
		},
	};
}

export async function listKubeContexts(
	client: TauriClient,
): Promise<ClusterContext[]> {
	return client.invoke<ClusterContext[]>("list_kube_contexts");
}

export async function listNamespaces(
	client: TauriClient,
	clusterContext: string,
): Promise<NamespaceSummary[]> {
	return client.invoke<NamespaceSummary[]>("list_namespaces", {
		clusterContext,
	});
}

export async function listResourceKinds(
	client: TauriClient,
	clusterContext: string,
): Promise<DiscoveredResourceKind[]> {
	return client.invoke<DiscoveredResourceKind[]>("list_resource_kinds", {
		clusterContext,
	});
}

export async function listResources(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	namespace?: string,
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_resources", {
		clusterContext,
		kind,
		namespace,
	});
}

export async function listDynamicResources(
	client: TauriClient,
	clusterContext: string,
	resourceKind: DiscoveredResourceKind,
	namespace?: string,
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_dynamic_resources", {
		clusterContext,
		resourceKind,
		namespace,
	});
}

export async function listResourceScope(
	client: TauriClient,
	clusterContext: string,
	requests: ResourceListRequest[],
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_resource_scope", {
		clusterContext,
		requests,
	});
}

export async function getResourceYaml(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
): Promise<string> {
	return client.invoke<string>("get_resource_yaml", {
		clusterContext,
		kind,
		name,
		namespace,
	});
}

export async function getResourceDetails(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_resource_details", {
		clusterContext,
		kind,
		name,
		namespace,
	});
}

export async function getDynamicResourceDetails(
	client: TauriClient,
	clusterContext: string,
	resourceKind: DiscoveredResourceKind,
	name: string,
	namespace?: string,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_dynamic_resource_details", {
		clusterContext,
		resourceKind,
		name,
		namespace,
	});
}

export async function listResourceEvents(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
): Promise<ResourceEventSummary[]> {
	return client.invoke<ResourceEventSummary[]>("list_resource_events", {
		clusterContext,
		kind,
		name,
		namespace,
	});
}

export async function listResourceTopology(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
	mode: TopologyMode = "ownership",
): Promise<ResourceTopology> {
	return client.invoke<ResourceTopology>("list_resource_topology", {
		clusterContext,
		namespaces,
		mode,
	});
}

export function createStreamChannel(
	onMessage: (message: StreamMessage) => void,
): Channel<StreamMessage> {
	return new Channel<StreamMessage>(onMessage);
}

function closeChannel<T>(channel: Channel<T>): void {
	const disposableChannel = channel as unknown as {
		cleanupCallback?: () => void;
		unregister?: () => Promise<void>;
	};
	if (typeof disposableChannel.unregister === "function") {
		void disposableChannel.unregister();
		return;
	}
	disposableChannel.cleanupCallback?.();
}

export function closeStreamChannel(channel: Channel<StreamMessage>): void {
	closeChannel(channel);
}

export function createPodExecChannel(
	onMessage: (message: PodExecSessionMessage) => void,
): Channel<PodExecSessionMessage> {
	return new Channel<PodExecSessionMessage>(onMessage);
}

export function closePodExecChannel(
	channel: Channel<PodExecSessionMessage>,
): void {
	closeChannel(channel);
}

export async function startResourceWatch(
	client: TauriClient,
	clusterContext: string,
	keys: WatchResourceKey[],
	channel: Channel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_resource_watch", {
		clusterContext,
		keys,
		channel,
	});
}

export async function startResourceEventWatch(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace: string | null | undefined,
	channel: Channel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_resource_event_watch", {
		clusterContext,
		kind,
		name,
		namespace,
		channel,
	});
}

export async function startPodLogStream(
	client: TauriClient,
	request: PodLogStreamRequest,
	channel: Channel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_pod_log_stream", { request, channel });
}

export async function stopStream(
	client: TauriClient,
	streamId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_stream", { streamId });
}

export async function startPortForward(
	client: TauriClient,
	request: PortForwardRequest,
): Promise<PortForwardSessionSummary> {
	return client.invoke<PortForwardSessionSummary>("start_pod_port_forward", {
		request,
	});
}

export async function startPodPortForward(
	client: TauriClient,
	request: PortForwardRequest,
): Promise<PortForwardSessionSummary> {
	return startPortForward(client, request);
}

export async function stopPodPortForward(
	client: TauriClient,
	sessionId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_port_forward", { sessionId });
}

export async function listPortForwards(
	client: TauriClient,
): Promise<PortForwardSessionSummary[]> {
	return client.invoke<PortForwardSessionSummary[]>("list_port_forwards");
}

export async function startPodExecSession(
	client: TauriClient,
	request: PodExecSessionRequest,
	channel: Channel<PodExecSessionMessage>,
): Promise<PodExecSessionSummary> {
	return client.invoke<PodExecSessionSummary>("start_pod_exec_session", {
		request,
		channel,
	});
}

export async function writePodExecStdin(
	client: TauriClient,
	sessionId: string,
	data: string,
): Promise<boolean> {
	return client.invoke<boolean>("write_pod_exec_stdin", { sessionId, data });
}

export async function resizePodExecTerminal(
	client: TauriClient,
	sessionId: string,
	size: PodExecTerminalSize,
): Promise<boolean> {
	return client.invoke<boolean>("resize_pod_exec_terminal", { sessionId, size });
}

export async function stopPodExecSession(
	client: TauriClient,
	sessionId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_pod_exec_session", { sessionId });
}

export async function listPodExecSessions(
	client: TauriClient,
): Promise<PodExecSessionSummary[]> {
	return client.invoke<PodExecSessionSummary[]>("list_pod_exec_sessions");
}

export async function getAppUsageMetrics(
	client: TauriClient,
): Promise<AppUsageMetrics> {
	return client.invoke<AppUsageMetrics>("get_app_usage_metrics");
}

export async function listResourceMetrics(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
): Promise<ResourceMetricsSummary> {
	return client.invoke<ResourceMetricsSummary>("list_resource_metrics", {
		clusterContext,
		namespaces,
	});
}

export function isAppError(value: unknown): value is AppError {
	return (
		typeof value === "object" &&
		value !== null &&
		"message" in value &&
		"kind" in value
	);
}

// Argo CD commands
export async function detectArgoCD(
	client: TauriClient,
	clusterContext: string,
): Promise<boolean> {
	return client.invoke<boolean>("detect_argocd", { clusterContext });
}

export async function listArgoApplications(
	client: TauriClient,
	clusterContext: string,
): Promise<ArgoApplicationSummary[]> {
	return client.invoke<ArgoApplicationSummary[]>("list_argocd_applications", {
		clusterContext,
	});
}

export async function getArgoApplicationDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
): Promise<ArgoApplicationDetails> {
	return client.invoke<ArgoApplicationDetails>(
		"get_argocd_application_details",
		{ clusterContext, name, namespace },
	);
}

export async function listArgoApplicationSets(
	client: TauriClient,
	clusterContext: string,
): Promise<ArgoApplicationSetSummary[]> {
	return client.invoke<ArgoApplicationSetSummary[]>("list_argocd_appsets", {
		clusterContext,
	});
}

export async function listArgoAppProjects(
	client: TauriClient,
	clusterContext: string,
): Promise<ArgoAppProjectSummary[]> {
	return client.invoke<ArgoAppProjectSummary[]>("list_argocd_appprojects", {
		clusterContext,
	});
}

export async function getArgoApplicationSetDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
): Promise<ArgoApplicationSetDetails> {
	return client.invoke<ArgoApplicationSetDetails>("get_argocd_appset_details", {
		clusterContext,
		name,
		namespace,
	});
}

export async function getArgoAppProjectDetails(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace?: string,
): Promise<ArgoAppProjectDetails> {
	return client.invoke<ArgoAppProjectDetails>("get_argocd_appproject_details", {
		clusterContext,
		name,
		namespace,
	});
}

export async function listHelmReleases(
	client: TauriClient,
	clusterContext: string,
): Promise<HelmReleaseSummary[]> {
	return client.invoke<HelmReleaseSummary[]>("list_helm_releases", {
		clusterContext,
	});
}

export async function getHelmReleaseDetails(
	client: TauriClient,
	release: Pick<
		HelmReleaseSummary,
		"cluster" | "namespace" | "storageKind" | "storageName"
	>,
): Promise<HelmReleaseDetails> {
	return client.invoke<HelmReleaseDetails>("get_helm_release_details", {
		clusterContext: release.cluster,
		namespace: release.namespace,
		storageKind: release.storageKind,
		storageName: release.storageName,
	});
}

export async function listRbacInspection(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
): Promise<RbacInspectionSummary> {
	return client.invoke<RbacInspectionSummary>("list_rbac_inspection", {
		clusterContext,
		namespaces,
	});
}

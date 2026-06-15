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
	YamlApplyPreview,
	YamlApplyRequest,
	YamlApplyResult,
	YamlEncoding,
	YamlViewMode,
	KubernetesYamlLintResult,
	KubeconfigSourcesSummary,
	LiveSessionCleanupRequest,
	LiveSessionCleanupResult,
	CancellableRequest,
} from "./types";
import { diagnosticLog, diagnosticResultSummary } from "./diagnostics";

export interface TauriClient {
	invoke<T>(
		cmd: string,
		args?: Record<string, unknown>,
		options?: InvokeOptions,
	): Promise<T>;
}

function errorMessage(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;
	if (
		typeof error === "object" &&
		error !== null &&
		"message" in error &&
		typeof error.message === "string"
	) {
		return error.message;
	}
	return String(error);
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
					error: errorMessage(error),
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

function cancellableArg(
	request?: CancellableRequest,
): Partial<CancellableRequest> {
	if (!request) return {};
	return request;
}

function sanitizeKubeconfigRequest<T extends { kubeconfigEnvVar?: string }>(
	request: T,
): T {
	if (!request.kubeconfigEnvVar?.startsWith("kubeconfigSource=")) return request;
	const { kubeconfigEnvVar: _ignored, ...rest } = request;
	return rest as T;
}

export async function getKubeconfigSources(
	client: TauriClient,
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("get_kubeconfig_sources");
}

export async function setKubeconfigEnvVar(
	client: TauriClient,
	envVar: string,
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("set_kubeconfig_env_var", {
		envVar,
	});
}

export async function setShowKubeconfigSourceLabels(
	client: TauriClient,
	show: boolean,
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>(
		"set_show_kubeconfig_source_labels",
		{ show },
	);
}

export async function pickKubeconfigPaths(
	client: TauriClient,
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("pick_kubeconfig_paths");
}

export async function addKubeconfigPaths(
	client: TauriClient,
	paths: string[],
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("add_kubeconfig_paths", {
		paths,
	});
}

export async function removeKubeconfigPath(
	client: TauriClient,
	path: string,
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("remove_kubeconfig_path", {
		path,
	});
}

export async function reorderKubeconfigPaths(
	client: TauriClient,
	paths: string[],
): Promise<KubeconfigSourcesSummary> {
	return client.invoke<KubeconfigSourcesSummary>("reorder_kubeconfig_paths", {
		paths,
	});
}

export async function listKubeContexts(
	client: TauriClient,
	kubeconfigEnvVar?: string,
): Promise<ClusterContext[]> {
	return client.invoke<ClusterContext[]>(
		"list_kube_contexts",
		kubeconfigArg(kubeconfigEnvVar),
	);
}

export async function listNamespaces(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<NamespaceSummary[]> {
	return client.invoke<NamespaceSummary[]>("list_namespaces", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listResourceKinds(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<DiscoveredResourceKind[]> {
	return client.invoke<DiscoveredResourceKind[]>("list_resource_kinds", {
		clusterContext,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listResources(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_resources", {
		clusterContext,
		kind,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listDynamicResources(
	client: TauriClient,
	clusterContext: string,
	resourceKind: DiscoveredResourceKind,
	namespace?: string,
	kubeconfigEnvVar?: string,
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_dynamic_resources", {
		clusterContext,
		resourceKind,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function listResourceScope(
	client: TauriClient,
	clusterContext: string,
	requests: ResourceListRequest[],
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<ResourceSummary[]> {
	return client.invoke<ResourceSummary[]>("list_resource_scope", {
		clusterContext,
		requests,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
	});
}

export async function getResourceYaml(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
	cancellable?: CancellableRequest,
): Promise<string> {
	return client.invoke<string>("get_resource_yaml", {
		clusterContext,
		kind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
		...cancellableArg(cancellable),
	});
}

export async function getResourceDetails(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
	cancellable?: CancellableRequest,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_resource_details", {
		clusterContext,
		kind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
		...cancellableArg(cancellable),
	});
}

export async function getDynamicResourceDetails(
	client: TauriClient,
	clusterContext: string,
	resourceKind: DiscoveredResourceKind,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	yamlViewMode?: YamlViewMode,
	yamlEncoding?: YamlEncoding,
	cancellable?: CancellableRequest,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_dynamic_resource_details", {
		clusterContext,
		resourceKind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
		...cancellableArg(cancellable),
	});
}

export async function prepareYamlApply(
	client: TauriClient,
	request: YamlApplyRequest,
): Promise<YamlApplyPreview> {
	return client.invoke<YamlApplyPreview>("prepare_yaml_apply", { request });
}

export async function applyYaml(
	client: TauriClient,
	request: YamlApplyRequest,
): Promise<YamlApplyResult> {
	return client.invoke<YamlApplyResult>("apply_yaml", { request });
}

export async function lintKubernetesYaml(
	client: TauriClient,
	request: YamlApplyRequest,
): Promise<KubernetesYamlLintResult> {
	return client.invoke<KubernetesYamlLintResult>("lint_kubernetes_yaml", {
		request,
	});
}

export async function listResourceEvents(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<ResourceEventSummary[]> {
	return client.invoke<ResourceEventSummary[]>("list_resource_events", {
		clusterContext,
		kind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
	});
}

export async function listResourceTopology(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
	mode: TopologyMode = "ownership",
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<ResourceTopology> {
	return client.invoke<ResourceTopology>("list_resource_topology", {
		clusterContext,
		namespaces,
		mode,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
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
	kubeconfigEnvVar?: string,
): Promise<string> {
	return client.invoke<string>("start_resource_watch", {
		clusterContext,
		keys,
		channel,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function startResourceEventWatch(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace: string | null | undefined,
	channel: Channel<StreamMessage>,
	kubeconfigEnvVar?: string,
): Promise<string> {
	return client.invoke<string>("start_resource_event_watch", {
		clusterContext,
		kind,
		name,
		namespace,
		channel,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function startPodLogStream(
	client: TauriClient,
	request: PodLogStreamRequest,
	channel: Channel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_pod_log_stream", {
		request: sanitizeKubeconfigRequest(request),
		channel,
	});
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
		request: sanitizeKubeconfigRequest(request),
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
		request: sanitizeKubeconfigRequest(request),
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

export async function stopLiveSessionsOutsideScope(
	client: TauriClient,
	request: LiveSessionCleanupRequest,
): Promise<LiveSessionCleanupResult> {
	return client.invoke<LiveSessionCleanupResult>(
		"stop_live_sessions_outside_scope",
		{ request },
	);
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
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
): Promise<ResourceMetricsSummary> {
	return client.invoke<ResourceMetricsSummary>("list_resource_metrics", {
		clusterContext,
		namespaces,
		...kubeconfigArg(kubeconfigEnvVar),
		...cancellableArg(cancellable),
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

export * from "./tauri-inspection";
export * from "./tauri-diagnostics";

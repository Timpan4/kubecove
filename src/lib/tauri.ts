import { invoke, type InvokeOptions } from "@tauri-apps/api/core";
import { createDevMockTauriClient } from "./tauri-dev-mocks";
import { cancellableArg, kubeconfigArg } from "./tauri-args";
import type {
	ClusterContext,
	NamespaceSummary,
	DiscoveredResourceKind,
	ResourceListRequest,
	ResourceSummary,
	ResourceEventSummary,
	ResourceDetailsFull,
	ResourceTopology,
	TopologyMode,
	AppError,
	AppUsageMetrics,
	ResourceMetricsSummary,
	YamlApplyPreview,
	YamlApplyRequest,
	YamlApplyResult,
	YamlEncoding,
	YamlViewMode,
	ScaleWorkloadRequest,
	RolloutRestartRequest,
	DeleteResourceRequest,
	ClusterOperationPreview,
	ClusterOperationResult,
	KubernetesYamlLintResult,
	KubeconfigSourcesSummary,
	CancellableRequest,
	DeploymentRevision,
} from "./types";
import { diagnosticLog, diagnosticResultSummary } from "./diagnostics";

export {
	createMockChannel,
	createMockTauriClient,
	isBrowserDevMockMode,
	isTauriRuntime,
	shouldUseBrowserDevMocks,
} from "./tauri-runtime";
export type { TauriClient } from "./tauri-runtime";
export { kubeconfigArg } from "./tauri-args";
export * from "./tauri-argo";
export * from "./tauri-streams";
import { shouldUseBrowserDevMocks } from "./tauri-runtime";
import type { TauriClient } from "./tauri-runtime";

const inFlightInvokes = new WeakMap<TauriClient, Map<string, Promise<unknown>>>();

function sortedScopeKey(values: string[]): string {
	return [...new Set(values)].sort((a, b) => a.localeCompare(b)).join(",");
}

function coalescedInvoke<T>(
	client: TauriClient,
	key: string,
	invokeCommand: () => Promise<T>,
): Promise<T> {
	let clientInvokes = inFlightInvokes.get(client);
	if (!clientInvokes) {
		clientInvokes = new Map();
		inFlightInvokes.set(client, clientInvokes);
	}
	const existing = clientInvokes.get(key);
	if (existing) return existing as Promise<T>;

	const request = invokeCommand().finally(() => {
		clientInvokes.delete(key);
	});
	clientInvokes.set(key, request);
	return request;
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
	if (shouldUseBrowserDevMocks()) return createDevMockTauriClient();

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

export async function saveWorkspaceExportJson(
	client: TauriClient,
	fileName: string,
	content: string,
): Promise<boolean> {
	return client.invoke<boolean>("save_workspace_export_json", {
		fileName,
		content,
	});
}

export async function pickWorkspaceImportJson(
	client: TauriClient,
): Promise<string | null> {
	return client.invoke<string | null>("pick_workspace_import_json");
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
	return coalescedInvoke(
		client,
		`list_namespaces:${kubeconfigEnvVar ?? ""}:${clusterContext}`,
		() =>
			client.invoke<NamespaceSummary[]>("list_namespaces", {
				clusterContext,
				...kubeconfigArg(kubeconfigEnvVar),
			}),
	);
}

export async function listResourceKinds(
	client: TauriClient,
	clusterContext: string,
	kubeconfigEnvVar?: string,
): Promise<DiscoveredResourceKind[]> {
	return coalescedInvoke(
		client,
		`list_resource_kinds:${kubeconfigEnvVar ?? ""}:${clusterContext}`,
		() =>
			client.invoke<DiscoveredResourceKind[]>("list_resource_kinds", {
				clusterContext,
				...kubeconfigArg(kubeconfigEnvVar),
			}),
	);
}

export async function listPresentCustomResourceKinds(
	client: TauriClient,
	clusterContext: string,
	namespaces: string[],
	kubeconfigEnvVar?: string,
): Promise<DiscoveredResourceKind[]> {
	return coalescedInvoke(
		client,
		`list_present_custom_resource_kinds:${kubeconfigEnvVar ?? ""}:${clusterContext}:${sortedScopeKey(namespaces)}`,
		() =>
			client.invoke<DiscoveredResourceKind[]>(
				"list_present_custom_resource_kinds",
				{
					clusterContext,
					namespaces,
					...kubeconfigArg(kubeconfigEnvVar),
				},
			),
	);
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

export async function listDeploymentRevisions(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace: string,
	kubeconfigEnvVar?: string,
): Promise<DeploymentRevision[]> {
	return client.invoke<DeploymentRevision[]>("list_deployment_revisions", {
		clusterContext,
		name,
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

export function revealSecretDataValue(
	client: TauriClient,
	clusterContext: string,
	name: string,
	namespace: string,
	key: string,
	kubeconfigEnvVar?: string,
): Promise<string> {
	return client.invoke<string>("reveal_secret_data_value", {
		clusterContext,
		name,
		namespace,
		key,
		...kubeconfigArg(kubeconfigEnvVar),
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
	redactSecrets?: boolean,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_dynamic_resource_details", {
		clusterContext,
		resourceKind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
		yamlViewMode,
		yamlEncoding,
		redactSecrets,
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

export async function previewScaleWorkload(
	client: TauriClient,
	request: ScaleWorkloadRequest,
): Promise<ClusterOperationPreview> {
	return client.invoke<ClusterOperationPreview>("preview_scale_workload", { request });
}

export async function scaleWorkload(
	client: TauriClient,
	request: ScaleWorkloadRequest,
): Promise<ClusterOperationResult> {
	return client.invoke<ClusterOperationResult>("scale_workload", { request });
}

export async function previewRolloutRestart(
	client: TauriClient,
	request: RolloutRestartRequest,
): Promise<ClusterOperationPreview> {
	return client.invoke<ClusterOperationPreview>("preview_rollout_restart", { request });
}

export async function rolloutRestart(
	client: TauriClient,
	request: RolloutRestartRequest,
): Promise<ClusterOperationResult> {
	return client.invoke<ClusterOperationResult>("rollout_restart", { request });
}

export async function previewDeleteResource(
	client: TauriClient,
	request: DeleteResourceRequest,
): Promise<ClusterOperationPreview> {
	return client.invoke<ClusterOperationPreview>("preview_delete_resource", { request });
}

export async function deleteResource(
	client: TauriClient,
	request: DeleteResourceRequest,
): Promise<ClusterOperationResult> {
	return client.invoke<ClusterOperationResult>("delete_resource", { request });
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
	if (cancellable) {
		return client.invoke<ResourceTopology>("list_resource_topology", {
			clusterContext,
			namespaces,
			mode,
			...kubeconfigArg(kubeconfigEnvVar),
			...cancellableArg(cancellable),
		});
	}
	return coalescedInvoke(
		client,
		`list_resource_topology:${kubeconfigEnvVar ?? ""}:${clusterContext}:${sortedScopeKey(namespaces)}:${mode}`,
		() =>
			client.invoke<ResourceTopology>("list_resource_topology", {
				clusterContext,
				namespaces,
				mode,
				...kubeconfigArg(kubeconfigEnvVar),
			}),
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

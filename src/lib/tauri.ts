import { Channel, type InvokeOptions } from "@tauri-apps/api/core";
import { IS_DEV_BUILD } from "./build-env";
import { cancellableArg, kubeconfigArg } from "./tauri-args";
import { createBrowserTauriClient } from "./browser-runtime";
import { getDesktopRemoteTauriClient, isDesktopRemoteSelected } from "./remote-runtime";
import type {
	ClusterContext,
	NamespaceSummary,
	DiscoveredResourceKind,
	ResourceListRequest,
	ResourceScopeResult,
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
	ClusterOperationResult,
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionStateRequest,
	ResourceActionState,
	KubernetesYamlLintResult,
	KubeconfigSourcesSummary,
	CancellableRequest,
	DeploymentRevision,
	WatchResourceKey,
} from "./types";
import { createNativeTauriClient } from "./tauri-runtime";

export {
	createMockChannel,
	createMockTauriClient,
	getChannelId,
	isBrowserRuntime,
	isBrowserDevMockMode,
	isTauriRuntime,
	shouldUseBrowserDevMocks,
} from "./tauri-runtime";
export type { TauriClient } from "./tauri-runtime";
export { getDesktopRemoteTauriClient, isDesktopRemoteSelected } from "./remote-runtime";
export { kubeconfigArg } from "./tauri-args";
export * from "./tauri-argo";
export * from "./tauri-streams";
import { isBrowserRuntime, shouldUseBrowserDevMocks } from "./tauri-runtime";
import type { TauriClient } from "./tauri-runtime";

const inFlightInvokes = new WeakMap<TauriClient, Map<string, Promise<unknown>>>();
const readGenerations = new Map<string, number>();

function sortedScopeKey(values: string[]): string {
	return [...new Set(values)].sort((a, b) => a.localeCompare(b)).join(",");
}

function coalescedInvoke<T>(
	client: TauriClient,
	key: string,
	invokeCommand: () => Promise<T>,
): Promise<T> {
	key = `${readGenerations.get(key) ?? 0}:${key}`;
	let clientInvokes = inFlightInvokes.get(client);
	if (!clientInvokes) {
		clientInvokes = new Map();
		inFlightInvokes.set(client, clientInvokes);
	}
	const existing = clientInvokes.get(key);
	if (existing) {
		// SAFETY: this key stores only promises created by this invocation's result contract T.
		return existing as Promise<T>;
	}

	const request = invokeCommand().finally(() => {
		clientInvokes.delete(key);
	});
	clientInvokes.set(key, request);
	return request;
}

export function refreshResourceCache(
	client: TauriClient,
	clusterContext: string,
	keys: WatchResourceKey[],
	namespaces: string[],
	kubeconfigEnvVar?: string,
): Promise<{ clearedEntries: number }> {
	const scope = `${kubeconfigEnvVar ?? ""}:${clusterContext}`;
	for (const key of [`list_namespaces:${scope}`, `list_resource_kinds:${scope}`, `list_present_custom_resource_kinds:${scope}:${sortedScopeKey(namespaces)}`]) {
		readGenerations.set(key, (readGenerations.get(key) ?? 0) + 1);
	}
	return client.invoke("refresh_resource_cache", { clusterContext, keys, namespaces, ...kubeconfigArg(kubeconfigEnvVar) });
}

const nativeTauriClient = createNativeTauriClient();
let createDevMockClient: (() => TauriClient) | undefined;

export async function initializeBrowserDevMocks(): Promise<void> {
	// Keep the define at the import site so production removes the entire mock graph.
	if (process.env.KUBECOVE_PUBLIC_DEV === "true" && shouldUseBrowserDevMocks()) {
		const mocks = await import("./tauri-dev-mocks");
		createDevMockClient = mocks.createDevMockTauriClient;
	}
}

export function createTauriClient(): TauriClient {
	if (isBrowserRuntime()) return createBrowserTauriClient();
	if (IS_DEV_BUILD && shouldUseBrowserDevMocks()) {
		if (!createDevMockClient) throw new Error("Browser dev mocks must initialize before mounting the app");
		return createDevMockClient();
	}
	return {
		createChannel: <T>(onMessage: (message: T) => void) => {
			const activeClient = isDesktopRemoteSelected()
				? getDesktopRemoteTauriClient()
				: nativeTauriClient;
			return activeClient.createChannel?.(onMessage) ?? new Channel(onMessage);
		},
		invoke: <T, Args extends object = object>(
			cmd: string,
			args?: Args,
			options?: InvokeOptions,
		) => {
			const activeClient = isDesktopRemoteSelected()
				? getDesktopRemoteTauriClient()
				: nativeTauriClient;
			return activeClient.invoke<T, Args>(cmd, args, options);
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
): Promise<ResourceScopeResult> {
	return client.invoke<ResourceScopeResult>("list_resource_scope", {
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
	cancellable?: CancellableRequest,
): Promise<ResourceDetailsFull> {
	return client.invoke<ResourceDetailsFull>("get_resource_details", {
		clusterContext,
		kind,
		name,
		namespace,
		...kubeconfigArg(kubeconfigEnvVar),
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

export async function getResourceActionState(
	client: TauriClient,
	request: ResourceActionStateRequest,
): Promise<ResourceActionState> {
	return client.invoke<ResourceActionState>("get_resource_action_state", { request });
}

export async function previewResourceAction(
	client: TauriClient,
	request: ResourceActionRequest,
): Promise<ResourceActionPreview> {
	return client.invoke<ResourceActionPreview>("preview_resource_action", { request });
}

export async function executeResourceAction(
	client: TauriClient,
	request: ResourceActionRequest,
): Promise<ClusterOperationResult> {
	return client.invoke<ClusterOperationResult>("execute_resource_action", { request });
}

export async function listResourceEvents(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace?: string,
	kubeconfigEnvVar?: string,
	cancellable?: CancellableRequest,
	uid?: string,
): Promise<ResourceEventSummary[]> {
	return client.invoke<ResourceEventSummary[]>("list_resource_events", {
		clusterContext,
		kind,
		name,
		namespace,
		uid,
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

export function isAppError<Value>(value: Value): value is Value & AppError {
	return (
		value !== null &&
		Object(value) === value &&
		"message" in Object(value) &&
		"kind" in Object(value)
	);
}

export * from "./tauri-inspection";
export * from "./tauri-diagnostics";

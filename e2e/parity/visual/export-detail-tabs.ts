// Exports the browser-mock responses the resource detail tabs read: log
// streams, Deployment revisions, selected-resource action state, dry-run
// previews and executions, and the YAML apply review. Requests are built as the
// clients send them, so the GPUI fixture can refuse any other request.
import { readResourceYaml } from "../../../src/features/resource-detail/resourceDetailReadSpec";
import { buildYamlApplyRequest } from "../../../src/features/resource-detail/yamlApplyModel";
import type { TauriClient } from "../../../src/lib/tauri";
import type {
	ClusterOperationResult,
	DeploymentRevision,
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionState,
	ResourceSummary,
	StreamMessage,
	YamlApplyPreview,
} from "../../../src/lib/types";

const ACTION_KINDS = new Set(["Deployment", "StatefulSet", "DaemonSet", "Pod", "ConfigMap"]);

async function streamMessages(client: TauriClient, command: string, request: object): Promise<StreamMessage[]> {
	const messages: StreamMessage[] = [];
	if (!client.createChannel) throw new Error("The mock client has no stream channel");
	const channel = client.createChannel<StreamMessage>((message) => messages.push(message));
	await client.invoke(command, { request, channel });
	// The mocks deliver each message on a 10 ms timer.
	await new Promise((resolve) => setTimeout(resolve, 100));
	return messages;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** The detail-tab fixture for `resources`; actions and YAML apply cover `focus`, the captured Deployment. */
export async function exportDetailTabs(client: TauriClient, resources: ResourceSummary[], focus: ResourceSummary) {
	const clusterContext = focus.cluster;
	const logs = {
		pod: await streamMessages(client, "start_pod_log_stream", { batchLines: true }),
		aggregate: await streamMessages(client, "start_aggregated_log_stream", { batchLines: true }),
	};
	const revisions = await client.invoke<DeploymentRevision[]>("list_deployment_revisions", { clusterContext, name: focus.name, namespace: focus.namespace });
	const target = (row: ResourceSummary) => ({ clusterContext: row.cluster, kind: row.kind, name: row.name, namespace: row.namespace ?? undefined, apiVersion: row.apiVersion });
	const states: ResourceActionState[] = [];
	for (const row of resources.filter((candidate) => ACTION_KINDS.has(candidate.kind) && !candidate.dynamic)) {
		states.push(await client.invoke<ResourceActionState>("get_resource_action_state", { request: target(row) }));
	}
	// Each review runs against a fresh mock object: executing changes its resourceVersion,
	// so every non-delete execution is followed by a reviewed delete that resets it.
	const previews: { request: ResourceActionRequest; preview: ResourceActionPreview }[] = [];
	const executions: { request: ResourceActionRequest; result: ClusterOperationResult }[] = [];
	const review = async (request: ResourceActionRequest) => {
		const preview = await client.invoke<ResourceActionPreview>("preview_resource_action", { request });
		const confirmed: ResourceActionRequest = { ...request, expectedUid: preview.uid, expectedResourceVersion: preview.resourceVersion, confirmed: true, ...(request.action === "rollback" && { expectedRevisionUid: preview.revisionUid, expectedRevisionResourceVersion: preview.revisionResourceVersion }) };
		const result = await client.invoke<ClusterOperationResult>("execute_resource_action", { request: confirmed });
		return { preview, confirmed, result };
	};
	const container = { name: "api", env: [], requests: {}, limits: {} };
	const requests: ResourceActionRequest[] = [
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, name: "sidecar", image: "registry.example/sidecar:v2" },{ ...container, env: [{name: "MODE", value: "merged"}] }] },confirmed:false },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, env: [{ name: "RENAMED", value: "new" }, { name: "MODE", remove: true }] }] }, confirmed: false },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, name: "sidecar", image: "registry.example/sidecar:v2" }] }, confirmed: false },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, env: [{ name: "MODE", value: "reviewed" }] }] }, confirmed: false },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, requests: { cpu: "100m" }, limits: { memory: "128Mi" } }] }, confirmed: false },
		{ ...target(focus), action: "edit", edits: { labels: { app: "reviewed" } }, confirmed: false },
		{ ...target(focus), action: "edit", edits: { annotations: { "mock.example/keep": "reviewed" } }, confirmed: false },
		{ ...target(focus), action: "rollback", revision: 2, confirmed: false },
		{ ...target(focus), action: "restart", confirmed: false },
		{ ...target(focus), action: "pause", confirmed: false },
		{ ...target(focus), action: "edit", edits: { replicas: 3 }, confirmed: false },
		{ ...target(focus), action: "delete", confirmed: false },
	];
	for (const request of requests) {
		const { preview, confirmed, result } = await review(request);
		previews.push({ request, preview });
		executions.push({ request: confirmed, result });
		if (request.action !== "delete") await review({ ...target(focus), action: "delete", confirmed: false });
	}
	const draft = await client.invoke<string>("get_resource_yaml", { clusterContext, kind: focus.kind, name: focus.name, namespace: focus.namespace, yamlViewMode: "applyClean" });
	const yamlRequest = buildYamlApplyRequest({ resource: focus, yaml: draft, yamlEncoding: "yaml", forceConflicts: false });
	const preview = await client.invoke<YamlApplyPreview>("prepare_yaml_apply", { request: yamlRequest });
	const applyError = await client.invoke("apply_yaml", { request: { clusterContext, kind: focus.kind, name: focus.name, namespace: focus.namespace, yaml: draft } }).then(
		() => { throw new Error("The browser mock unexpectedly applied YAML"); },
		errorMessage,
	);
	const yamlReads = [];
	for (const row of resources) {
		for (const mode of ["kubectl", "applyClean"] as const) for (const encoding of ["yaml", "kyaml"] as const) {
		    yamlReads.push({ target: target(row), mode, encoding, yaml: await readResourceYaml(client, row, { kubeconfigSourceKey: undefined, yamlViewMode: mode, yamlEncoding: encoding }) });
		}
	}
	const yamlApply = [{ request: yamlRequest, preview, applyError }];
	return { logs, revisions, actions: { states, previews, executions }, yamlApply, yamlReads };
}

// Exports the browser-mock responses the resource detail tabs read: log
// streams, Deployment revisions, selected-resource action state, dry-run
// previews and executions, and the YAML apply review. Requests are built as the
// clients send them, so the GPUI fixture can refuse any other request.
import { readResourceYaml } from "../../../src/features/resource-detail/resourceDetailReadSpec";
import { buildYamlApplyRequest } from "../../../src/features/resource-detail/yamlApplyModel";
import type { TauriClient } from "../../../src/lib/tauri";
import { emptyTemplateRevisions } from "../../../src/lib/tauri-dev-mock-data";
import type {
	ClusterOperationResult,
	DeploymentRevision,
	KubernetesYamlLintResult,
	ResourceActionConfirmation,
	ResourceActionKind,
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionState,
	ResourceSummary,
	StreamMessage,
	YamlApplyPreview,
} from "../../../src/lib/types";

const ACTION_KINDS = new Set(["Deployment", "StatefulSet", "DaemonSet", "Pod", "ConfigMap"]);
/** Drafts `yaml-lint-permissions.json` and `yaml-parser-diagnostics.json` type into the editor. Each must match its step exactly. */
const LINT_DRAFTS = [
	"apiVersion: apps/v1\nkind: StatefulSet\nmetadata:\n  name: payments-api\n  namespace: payments\n",
	"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: payments-api\n  namespace: payments\n",
	"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: payments-api\n  name: payments-api-v2\n  labels: !custom\n    app: payments\nspec: ]\n",
];

export async function streamMessages(client: TauriClient, command: string, request: { batchLines: boolean }): Promise<StreamMessage[]> {
	const messages: StreamMessage[] = [];
	if (!client.createChannel) throw new Error("The mock client has no stream channel");
	const channel = client.createChannel<StreamMessage>((message) => messages.push(message));
	await client.invoke(command, { request, channel });
	// The mocks deliver each message on a 10 ms timer.
	await new Promise((resolve) => setTimeout(resolve, 100));
	return messages;
}

function errorMessage<Value>(error: Value): string {
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
	const { name, namespace } = emptyTemplateRevisions;
	const revisionCases = [
		{ name, namespace, revisions: await client.invoke<DeploymentRevision[]>("list_deployment_revisions", { clusterContext, name, namespace }) },
	];
	const target = (row: ResourceSummary) => ({ clusterContext: row.cluster, kind: row.kind, name: row.name, namespace: row.namespace ?? undefined, apiVersion: row.apiVersion });
	const states: ResourceActionState[] = [];
	for (const row of resources.filter((candidate) => ACTION_KINDS.has(candidate.kind) && !candidate.dynamic)) {
		states.push(await client.invoke<ResourceActionState>("get_resource_action_state", { request: target(row) }));
	}
	// Each review runs against a fresh mock object: executing changes its resourceVersion,
	// so every non-delete execution is followed by a reviewed delete that resets it.
	const previews: { request: ResourceActionRequest; preview: ResourceActionPreview }[] = [];
	const executions: { confirmation: ResourceActionConfirmation; action: ResourceActionKind; result: ClusterOperationResult }[] = [];
	let pausedState: ResourceActionState | undefined;
	const review = async (request: ResourceActionRequest) => {
		const preview = await client.invoke<ResourceActionPreview>("preview_resource_action", { request });
		const confirmation = { reviewId: preview.reviewId };
		const result = await client.invoke<ClusterOperationResult>("execute_resource_action", { confirmation });
		return { preview, confirmation, result };
	};
	const container = { name: "api", env: [], requests: {}, limits: {} };
	const requests: ResourceActionRequest[] = [
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, name: "sidecar", image: "registry.example/sidecar:v2" },{ ...container, env: [{name: "MODE", value: "merged"}] }] } },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, env: [{ name: "RENAMED", value: "new" }, { name: "MODE", remove: true }] }] } },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, name: "sidecar", image: "registry.example/sidecar:v2" }] } },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, env: [{ name: "MODE", value: "reviewed" }] }] } },
		{ ...target(focus), action: "edit", edits: { containers: [{ ...container, requests: { cpu: "100m" }, limits: { memory: "128Mi" } }] } },
		{ ...target(focus), action: "edit", edits: { labels: { app: "reviewed" } } },
		{ ...target(focus), action: "edit", edits: { annotations: { "mock.example/keep": "reviewed" } } },
		{ ...target(focus), action: "rollback", revision: 2 },
		{ ...target(focus), action: "restart" },
		{ ...target(focus), action: "pause" },
		{ ...target(focus), action: "edit", edits: { replicas: 3 } },
		{ ...target(focus), action: "delete" },
	];
	for (const request of requests) {
		const { preview, confirmation, result } = await review(request);
		previews.push({ request, preview });
		executions.push({ confirmation, action: request.action, result });
		if (request.action === "pause") {
			pausedState = await client.invoke<ResourceActionState>("get_resource_action_state", { request: target(focus) });
			const resume: ResourceActionRequest = { ...target(focus), action: "resume" };
			const resumed = await review(resume);
			previews.push({ request: resume, preview: resumed.preview });
			executions.push({ confirmation: resumed.confirmation, action: resume.action, result: resumed.result });
		}
		if (request.action !== "delete") await review({ ...target(focus), action: "delete" });
	}
	// The managed-Pod scenario shows the owner without relying on replacement.
	const replica = resources.find((row) => row.cluster === focus.cluster && row.kind === "Pod" && row.name === "payments-api-7d9c9b7f8d-k2r9p");
	if (!replica) throw new Error("The browser mock has no ReplicaSet Pod payments-api-7d9c9b7f8d-k2r9p");
	const podDelete = await review({ ...target(replica), action: "delete" });
	previews.push({ request: { ...target(replica), action: "delete" }, preview: podDelete.preview });
	executions.push({ confirmation: podDelete.confirmation, action: "delete", result: podDelete.result });
	// Multi-context scenarios review the replica edit of the same Deployment in another context.
	const sameObject = (row: ResourceSummary) => row.cluster !== focus.cluster && row.kind === focus.kind && row.name === focus.name && row.namespace === focus.namespace;
	for (const row of resources.filter(sameObject)) {
		const request: ResourceActionRequest = { ...target(row), action: "edit", edits: { replicas: 3 } };
		previews.push({ request, preview: await client.invoke<ResourceActionPreview>("preview_resource_action", { request }) });
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
	const forcedRequest = { ...yamlRequest, forceConflicts: true };
	yamlApply.push({ request: forcedRequest, preview: await client.invoke<YamlApplyPreview>("prepare_yaml_apply", { request: forcedRequest }), applyError });
	// The editor lints the loaded draft and each typed draft, with the request the pane sends.
	const yamlLint = [];
	for (const yaml of [draft, ...LINT_DRAFTS]) {
		const request = buildYamlApplyRequest({ resource: focus, yaml, yamlEncoding: "yaml", forceConflicts: false });
		yamlLint.push({ request, result: await client.invoke<KubernetesYamlLintResult>("lint_kubernetes_yaml", { request }) });
	}
	return { logs, revisions, revisionCases, actions: { states, pausedState, previews, executions }, yamlApply, yamlLint, yamlReads };
}

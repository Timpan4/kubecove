import { stringify } from "yaml";
import type {
	ClusterOperationPreview,
	ClusterOperationResult,
	DeleteResourceRequest,
	ResourceActionPreview,
	ResourceActionRequest,
	ResourceActionState,
	ResourceActionStateRequest,
	RolloutRestartRequest,
	ScaleWorkloadRequest,
} from "./types";

interface MockArgs {
	request?: object;
}

type OperationRequest =
	| DeleteResourceRequest
	| ResourceActionRequest
	| RolloutRestartRequest
	| ScaleWorkloadRequest;

export const operationMockHandlers = {
	get_resource_action_state: (args?: MockArgs) => resourceActionState(resourceActionStateRequest(args)),
	preview_resource_action: (args?: MockArgs) => resourceActionPreview(resourceActionRequest(args)),
	execute_resource_action: (args?: MockArgs) => resourceActionResult(resourceActionRequest(args)),
	preview_scale_workload: (args?: MockArgs) =>
		scaleOperationPreview(operationRequest<ScaleWorkloadRequest>(args)),
	scale_workload: (args?: MockArgs) =>
		scaleOperationResult(operationRequest<ScaleWorkloadRequest>(args)),
	preview_rollout_restart: (args?: MockArgs) =>
		restartOperationPreview(operationRequest<RolloutRestartRequest>(args)),
	rollout_restart: (args?: MockArgs) =>
		restartOperationResult(operationRequest<RolloutRestartRequest>(args)),
	preview_delete_resource: (args?: MockArgs) =>
		deleteOperationPreview(operationRequest<DeleteResourceRequest>(args)),
	delete_resource: (args?: MockArgs) =>
		deleteOperationResult(operationRequest<DeleteResourceRequest>(args)),
};

function resourceActionRequest(args?: MockArgs): ResourceActionRequest {
	const request = args?.request;
	if (!request) throw new Error("Resource action request is required.");
	// SAFETY: each handler is called through its matching typed operation wrapper.
	return request as ResourceActionRequest;
}

function resourceActionStateRequest(args?: MockArgs): ResourceActionStateRequest {
	const request = args?.request;
	if (!request) throw new Error("Resource action state request is required.");
	// SAFETY: the state handler is called through its matching typed read wrapper.
	return request as ResourceActionStateRequest;
}

interface MockActionContainer {
	name: string;
	image: string;
	env?: { name: string; value?: string; valueFrom?: unknown }[];
	resources?: { requests?: Record<string, string>; limits?: Record<string, string> };
}

interface MockActionTemplate {
	metadata: { labels: Record<string, string>; annotations: Record<string, string> };
	spec: { containers: MockActionContainer[]; volumes?: unknown[] };
}

interface MockActionObject {
	apiVersion: string;
	kind: string;
	metadata: {
		name: string;
		namespace: string;
		uid: string;
		resourceVersion: string;
		labels: Record<string, string>;
		annotations: Record<string, string>;
		creationTimestamp?: string;
		generation?: number;
		managedFields?: unknown[];
		selfLink?: string;
	};
	spec?: {
		replicas?: number;
		paused?: boolean;
		selector?: { matchLabels: Record<string, string> };
		template?: MockActionTemplate;
		containers?: MockActionContainer[];
	};
	data?: Record<string, string>;
	status?: {
		replicas?: number;
		readyReplicas?: number;
		availableReplicas?: number;
		observedGeneration?: number;
	};
}

interface MockActionYamlObject {
	apiVersion: string;
	kind: string;
	metadata: {
		name: string;
		namespace: string;
		labels: Record<string, string>;
		annotations: Record<string, string>;
	};
	spec?: MockActionObject["spec"];
	data?: Record<string, string>;
}

// Synthetic resource objects live only in browser mock memory.
const mockActionObjects = new Map<string, MockActionObject>();

function resourceActionState(request: ResourceActionStateRequest): ResourceActionState {
	const object = getMockActionObject(request);
	const target = actionTarget(request);
	const actions = request.kind === "Deployment"
		? ["edit", "restart", "pause", "resume", "rollback", "delete"] as const
		: ["StatefulSet", "DaemonSet"].includes(request.kind)
			? ["edit", "restart", "delete"] as const
			: ["edit", "delete"] as const;
	const permissions = actions.map((action) => {
		const allowed = !deniedFor(request.name, action);
		return {
			action,
			allowed,
			reason: allowed ? "Allowed by browser mock SelfSubjectAccessReview." : `Browser mock denies ${action} for this fixture.`,
			requiredPermission: `${action === "delete" ? "delete" : "patch"} ${mockPlural(request.kind)}`,
		};
	});
	return {
		target,
		uid: object.metadata.uid,
		resourceVersion: object.metadata.resourceVersion,
		replicas: object.spec?.replicas,
		paused: object.spec?.paused,
		containers: mockContainers(object).map((container) => ({
			name: container.name,
			image: container.image,
			env: (container.env ?? []).map(({ name, value, valueFrom }) => ({ name, value, referenced: valueFrom !== undefined })),
			requests: Object.assign({}, container.resources?.requests),
			limits: Object.assign({}, container.resources?.limits),
		})),
		labels: { ...object.metadata.labels },
		annotations: { ...object.metadata.annotations },
		permissions,
		revisions: request.kind === "Deployment" && !request.name.includes("unowned") ? [
			{ revision: 3, images: ["registry.example/api:v1"] },
			{ revision: 2, images: ["registry.example/api:v0"] },
		] : [],
	};
}

function resourceActionPreview(request: ResourceActionRequest): ResourceActionPreview {
	assertActionAllowed(request);
	assertValidMockEdits(request);
	assertNotDryRunFailure(request.name);
	const before = getMockActionObject(request);
	assertMockGuard(request, before, false);
	const after = applyMockAction(cloneMock(before), request);
	const target = actionTarget(request);
	const revision = request.action === "rollback" ? mockRevisionIdentity(request) : undefined;
	return {
		target,
		effect: `${request.action} ${operationLabel(request)}`,
		beforeYaml: mockActionYaml(before),
		afterYaml: mockActionYaml(after),
		uid: before.metadata.uid,
		resourceVersion: before.metadata.resourceVersion,
		...(revision && {
			revisionUid: revision.uid,
			revisionResourceVersion: revision.resourceVersion,
		}),
	};
}

function resourceActionResult(request: ResourceActionRequest): ClusterOperationResult {
	requireMockConfirmation(request);
	assertActionAllowed(request);
	assertValidMockEdits(request);
	assertNotDryRunFailure(request.name);
	const object = getMockActionObject(request);
	assertMockGuard(request, object, true);
	if (request.action === "rollback") assertMockRevisionGuard(request);
	if (request.action === "delete") mockActionObjects.delete(mockActionKey(request));
	else {
		const applied = applyMockAction(cloneMock(object), request);
		applied.metadata.resourceVersion = String(Number(object.metadata.resourceVersion) + 1);
		mockActionObjects.set(mockActionKey(request), applied);
	}
	return {
		target: actionTarget(request),
		effect: `Simulated ${request.action} of ${operationLabel(request)} in browser mock mode.`,
	};
}

function actionTarget(request: ResourceActionStateRequest | ResourceActionRequest) {
	return {
		clusterContext: request.clusterContext,
		kind: request.kind,
		name: request.name,
		namespace: request.namespace,
		apiVersion: request.apiVersion,
	};
}

function mockActionKey(request: ResourceActionStateRequest): string {
	return [request.clusterContext, request.kind, request.namespace ?? "", request.name].join("\u0000");
}

function getMockActionObject(request: ResourceActionStateRequest): MockActionObject {
	const key = mockActionKey(request);
	const existing = mockActionObjects.get(key);
	if (existing) return existing;
	const deployment = request.kind === "Deployment";
	const workload = ["Deployment", "StatefulSet", "DaemonSet"].includes(request.kind);
	const apiVersion = request.apiVersion ?? (workload ? "apps/v1" : "v1");
	const container = {
		name: "api",
		image: "registry.example/api:v1",
		env: [
			{ name: "TOKEN", valueFrom: { secretKeyRef: { name: "mock-auth", key: "token" } } },
			{ name: "MODE", value: "safe" },
		],
		resources: { requests: { cpu: "25m" }, limits: { memory: "64Mi" } },
	};
	const template: MockActionTemplate = {
		metadata: { labels: { app: request.name }, annotations: { "mock.example/template": "preserved" } },
		spec: { containers: [container], volumes: [{ name: "data", emptyDir: {} }] },
	};
	const object: MockActionObject = {
		apiVersion,
		kind: request.kind,
		metadata: {
			name: request.name,
			namespace: request.namespace ?? "default",
			uid: "mock-uid-1",
			resourceVersion: "1",
			labels: { app: request.name, keep: "yes" },
			annotations: { "mock.example/keep": "yes" },
			creationTimestamp: "2026-01-01T00:00:00Z",
			generation: 1,
			managedFields: [{ manager: "browser-mock" }],
		},
	};
	if (workload) {
		object.spec = {
			replicas: request.kind === "DaemonSet" ? undefined : 2,
			paused: deployment ? false : undefined,
			selector: { matchLabels: { app: request.name } },
			template,
		};
	} else if (request.kind === "Pod") object.spec = { containers: [container] };
	else if (request.kind === "ConfigMap") object.data = { existing: "preserved" };
	mockActionObjects.set(key, object);
	return object;
}

function mockContainers(object: MockActionObject): MockActionContainer[] {
	return object.kind === "Pod" ? object.spec?.containers ?? [] : object.spec?.template?.spec.containers ?? [];
}

function deniedFor(name: string, action: ResourceActionRequest["action"]): boolean {
	return name.startsWith("mock-denied-") || name === `mock-denied-${action}`;
}

function assertActionAllowed(request: ResourceActionRequest): void {
	if (deniedFor(request.name, request.action)) throw new Error(`Browser mock denies ${request.action} for this fixture.`);
}

function assertNotDryRunFailure(name: string): void {
	if (name.startsWith("mock-dry-run-error")) throw new Error("Browser mock dry-run failed for this fixture.");
}

function assertMockGuard(request: ResourceActionRequest, object: MockActionObject, required: boolean): void {
	if (required && (!request.expectedUid || !request.expectedResourceVersion)) throw new Error("Reviewed UID and resourceVersion are required.");
	if (request.expectedUid !== undefined && request.expectedUid !== object.metadata.uid) throw new Error("Resource UID changed since preview.");
	if (request.expectedResourceVersion !== undefined && request.expectedResourceVersion !== object.metadata.resourceVersion) throw new Error("Resource version changed since preview.");
}

function mockRevisionIdentity(request: ResourceActionRequest) {
	const revision = request.revision;
	if (request.name.includes("unowned") || ![2, 3].includes(revision ?? 0)) {
		throw new Error("Revision is not owned by the selected Deployment.");
	}
	return { uid: `mock-revision-${revision}`, resourceVersion: "1" };
}

function assertMockRevisionGuard(request: ResourceActionRequest): void {
	const revision = mockRevisionIdentity(request);
	if (!request.expectedRevisionUid || !request.expectedRevisionResourceVersion) {
		throw new Error("Reviewed revision UID and resourceVersion are required.");
	}
	if (request.expectedRevisionUid !== revision.uid || request.expectedRevisionResourceVersion !== revision.resourceVersion) {
		throw new Error("Rollback revision changed since the action was reviewed.");
	}
}

function assertValidMockEdits(request: ResourceActionRequest): void {
	if (request.edits?.replicas !== undefined && request.edits.replicas < 0) throw new Error("Replicas must be zero or greater.");
	for (const container of request.edits?.containers ?? []) {
		if (container.image !== undefined && !container.image.trim()) throw new Error("Container image cannot be empty.");
	}
}

function applyMockAction(object: MockActionObject, request: ResourceActionRequest): MockActionObject {
	const spec = object.spec ?? {};
	switch (request.action) {
		case "edit": {
			const edits = request.edits ?? {};
			if (edits.replicas !== undefined) spec.replicas = edits.replicas;
			for (const [key, value] of Object.entries(edits.labels ?? {})) {
				if (value === null) delete object.metadata.labels[key];
				else object.metadata.labels[key] = value;
			}
			for (const [key, value] of Object.entries(edits.annotations ?? {})) {
				if (value === null) delete object.metadata.annotations[key];
				else object.metadata.annotations[key] = value;
			}
			for (const edit of edits.containers ?? []) {
				const container = mockContainers(object).find((candidate) => candidate.name === edit.name);
				if (!container) throw new Error(`Container ${edit.name} was not found.`);
				if (edit.image !== undefined) container.image = edit.image;
				if (edit.env?.length) {
					container.env ??= [];
					for (const change of edit.env) {
						const index = container.env.findIndex((entry) => entry.name === change.name);
						if (change.remove) { if (index >= 0) container.env.splice(index, 1); continue; }
						const entry = index >= 0 ? container.env[index] : { name: change.name };
						delete entry.valueFrom;
						entry.value = change.value ?? "";
						if (index < 0) container.env.push(entry);
					}
				}
				for (const field of ["requests", "limits"] as const) {
					const delta = edit[field];
					if (!delta) continue;
					container.resources ??= {};
					container.resources[field] ??= {};
					const quantities = container.resources[field];
					if (!quantities) continue;
					for (const [key, value] of Object.entries(delta)) {
						if (value === null) delete quantities[key];
						else quantities[key] = value;
					}
				}
			}
			object.spec = spec;
			break;
		}
		case "restart":
			if (!spec.template) throw new Error("Restart is unsupported for this fixture.");
			spec.template.metadata.annotations["kubecove.io/restartedAt"] = "mock-restart-1";
			break;
		case "pause": spec.paused = true; break;
		case "resume": spec.paused = false; break;
		case "rollback": {
			const revision = request.revision ?? 0;
			if (request.name.includes("unowned") || ![2, 3].includes(revision)) throw new Error("Revision is not owned by the selected Deployment.");
			spec.template = mockRevisionTemplate(request.name, revision);
			break;
		}
		case "delete": break;
	}
	return object;
}

function mockActionYaml(object: MockActionObject): string {
	const clean = cloneMock(object);
	const visible: MockActionYamlObject = {
		apiVersion: clean.apiVersion,
		kind: clean.kind,
		metadata: {
			name: clean.metadata.name,
			namespace: clean.metadata.namespace,
			labels: clean.metadata.labels,
			annotations: clean.metadata.annotations,
		},
	};
	if (clean.spec) visible.spec = clean.spec;
	if (clean.data) visible.data = clean.data;
	return `${stringify(visible)}\n`;
}

function mockRevisionTemplate(name: string, revision: number): MockActionTemplate {
	const version = revision === 2 ? "v0" : "v1";
	return {
		metadata: {
			labels: { app: name, revision: String(revision) },
			annotations: { "mock.example/template": `revision-${revision}` },
		},
		spec: {
			containers: [{
				name: "api",
				image: `registry.example/api:${version}`,
				env: [{ name: "MODE", value: `revision-${revision}` }],
				resources: { requests: { cpu: "50m" }, limits: { memory: "128Mi" } },
			}],
			volumes: [{ name: "config", configMap: { name: `${name}-revision-${revision}` } }],
		},
	};
}

function mockPlural(kind: string): string {
	return new Map([
		["Deployment", "deployments"],
		["StatefulSet", "statefulsets"],
		["DaemonSet", "daemonsets"],
		["Pod", "pods"],
		["ConfigMap", "configmaps"],
	]).get(kind) ?? "resources";
}

function cloneMock<T>(value: T): T {
	return structuredClone(value);
}

function operationRequest<T extends OperationRequest>(args?: MockArgs): T {
	const request = args?.request;
	if (!request) {
		throw new Error("Operation request is required.");
	}
	// SAFETY: each handler is called through its matching typed operation wrapper.
	return request as T;
}

function operationTarget(request: OperationRequest) {
	return {
		clusterContext: request.clusterContext,
		namespace: request.namespace,
		kind: request.kind,
		name: request.name,
	};
}

function operationLabel(request: OperationRequest) {
	return `${request.kind} ${request.namespace}/${request.name} in context ${request.clusterContext}`;
}

function requireMockConfirmation(request: OperationRequest) {
	if (!request.confirmed) throw new Error("Explicit confirmation is required.");
}

function scaleOperationPreview(
	request: ScaleWorkloadRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Scale ${operationLabel(request)} to ${request.replicas} replicas`,
	};
}

function scaleOperationResult(request: ScaleWorkloadRequest): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated scaling ${operationLabel(request)} to ${request.replicas} replicas in browser mock mode.`,
	};
}

function restartOperationPreview(
	request: RolloutRestartRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Restart ${operationLabel(request)} by updating its pod template`,
	};
}

function restartOperationResult(
	request: RolloutRestartRequest,
): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated restart of ${operationLabel(request)} in browser mock mode.`,
	};
}

function deleteOperationPreview(
	request: DeleteResourceRequest,
): ClusterOperationPreview {
	return {
		target: operationTarget(request),
		effect: `Delete ${operationLabel(request)}`,
	};
}

function deleteOperationResult(
	request: DeleteResourceRequest,
): ClusterOperationResult {
	requireMockConfirmation(request);
	return {
		target: operationTarget(request),
		effect: `Simulated deletion of ${operationLabel(request)} in browser mock mode.`,
	};
}

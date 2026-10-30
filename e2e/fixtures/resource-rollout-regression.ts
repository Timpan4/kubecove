import { QueryClient } from "@tanstack/svelte-query";
import { mount, unmount } from "svelte";
import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
import { createMockTauriClient } from "@/lib/tauri-runtime";
import type { ResourceListRequest, ResourceSummary, ResourceTopology, StreamMessage } from "@/lib/types";
import ResourceRolloutHarness from "./ResourceRolloutHarness.svelte";

type WatchChannel = { onmessage: (message: StreamMessage) => void };
let control: { add(): void; fail(): void; complete(): void; switchScope(): void; reconnect(): void; finishReads(): void; stop(): Promise<void> } | undefined;

export async function mountResourceRollout() {
	await control?.stop();
	let completed = false;
	let added = false;
	let failing = false;
	let dataNamespace = "layercove";
	let readGate: Promise<void> | undefined;
	let finishReads = () => {};
	const holdReads = () => {
		readGate = new Promise<void>((resolve) => { finishReads = () => { readGate = undefined; resolve(); }; });
	};
	const channels: WatchChannel[] = [];
	const row = (kind: string, name: string, status: string, ready: string, ownerRef?: string): ResourceSummary => ({
		cluster: "rollout-regression", namespace: dataNamespace, kind, name, status, ready,
		age: kind === "Deployment" ? "93d" : "11m", health: status === "Running" || status.startsWith("Available") ? "healthy" : "attention",
		argoApp: "layercove", gitOpsOwner: { provider: "argo", kind: "Application", name: "layercove", confidence: "metadata", partial: true }, ownerRef,
		healthAssessment: {
			state: kind === "ConfigMap" ? "notEvaluated" : ready === "0/0" ? "unknown" : status === "Running" || status.startsWith("Available") ? "healthy" : "needsAttention",
			completeness: "complete", winningSources: ["kubernetes"], reasons: [status || "No workload health contract"],
			evidence: [{ source: "kubernetes", raw: status, current: true, reason: status || "No workload health contract" }],
		},
	});
	const snapshot = () => [
		row("Deployment", "layercove", completed ? "Available: 1" : "Rolling out", "1/1"),
		row("ReplicaSet", "layercove-new", completed ? "Available: 1" : "Starting", completed ? "1/1" : "0/1", "Deployment/layercove"),
		row("ReplicaSet", "layercove-old", "Available: 0", "0/0", "Deployment/layercove"),
		...added || completed ? [row("Pod", "layercove-new-pod", completed ? "Running" : "Starting", completed ? "True" : "False", "ReplicaSet/layercove-new")] : [],
		...completed ? [] : [row("Pod", "layercove-old-pod", "Terminating", "True", "ReplicaSet/layercove-old")],
		{ ...row("ConfigMap", "layercove-config", "", ""), health: "unknown" as const },
	];
	const topology = (): ResourceTopology => {
		const resources = snapshot();
		return {
			nodes: resources.map((summary) => ({ id: `${summary.kind}/${summary.name}`, ...summary, selectable: true, summary })),
			edges: resources.flatMap((summary) => summary.ownerRef ? [{ id: summary.name, source: summary.ownerRef, target: `${summary.kind}/${summary.name}`, relation: "owns" as const }] : []),
			warnings: [],
		};
	};
	const changed = (action: "added" | "deleted" = "deleted") => {
		for (const channel of channels) channel.onmessage({ type: "resourceChanged", streamId: "rollout", action, target: { cluster: "rollout-regression", namespace: "layercove", kind: "Pod", name: action === "added" ? "layercove-new-pod" : "layercove-old-pod" } });
	};
	const client = createMockTauriClient({
		list_resource_scope: async ({ requests }: { requests: ResourceListRequest[] }) => {
			await readGate;
			if (failing) throw new Error("Rollout list temporarily unavailable");
			return { resources: snapshot(), completedRequests: requests, failures: [] };
		},
		list_resource_topology: async () => {
			await readGate;
			if (failing) throw new Error("Rollout topology temporarily unavailable");
			return topology();
		},
		start_resource_watch: ({ channel }: { channel: WatchChannel }) => {
			channels.push(channel);
			queueMicrotask(() => channel.onmessage({ type: "status", streamId: "rollout", status: "connected", message: "Connected" }));
			return `rollout-${channels.length}`;
		},
		list_namespaces: [{ name: "layercove", age: "93d" }, { name: "rollout-next", age: "1d" }], list_resource_kinds: [],
		list_resource_metrics: { cluster: "rollout-regression", availability: { status: "noSamples" }, pods: [], nodes: [], workloads: [], warnings: [] },
		refresh_resource_cache: { clearedEntries: 1 }, stop_stream: true,
		cancel_backend_requests: { cancelled: 0 },
	});
	const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	const target = document.createElement("div");
	document.body.append(target);
	const component = mount(ResourceRolloutHarness, { target, props: {
		queryClient, client, title: "Rollout regression", initialNamespaces: ["layercove"],
		initialKinds: ["Deployment", "ReplicaSet", "Pod", "ConfigMap"], customResourcesEnabled: false,
		workspaceReadContext: { workspaceId: "rollout-regression", clusterContext: "rollout-regression", namespaceScope: [], sourceReady: true, sourceError: null, showKubeconfigSourceLabels: false },
	} });
	control = {
		switchScope() {
			holdReads();
			dataNamespace = "rollout-next";
			component.changeNamespace(dataNamespace);
		},
		reconnect() {
			holdReads();
			for (const channel of channels) channel.onmessage({ type: "error", streamId: "rollout", message: "Watch interrupted" });
			for (const channel of channels) channel.onmessage({ type: "status", streamId: "rollout", status: "connected", message: "Connected" });
		},
		finishReads() { finishReads(); },
		add() { added = true; changed("added"); },
		fail() { failing = true; changed(); },
		complete() { failing = false; completed = true; },
		async stop() { finishReads(); await unmount(component); queryClient.clear(); target.remove(); control = undefined; },
	};
	return { recoveryInterval: RESOURCE_RECOVERY_INTERVAL };
}

export function failResourceRollout() { control?.fail(); }
export function addResourceRolloutPod() { control?.add(); }
export function switchResourceRolloutScope() { control?.switchScope(); }
export function reconnectResourceRollout() { control?.reconnect(); }
export function finishResourceRolloutReads() { control?.finishReads(); }
// No event is emitted here. The recovery timer must replace the failed snapshot.
export function completeResourceRollout() { control?.complete(); }
export async function stopResourceRollout() { await control?.stop(); }

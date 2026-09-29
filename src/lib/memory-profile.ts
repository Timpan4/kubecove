import type { Query, QueryClient } from "@tanstack/svelte-query";

const ROOT_CLASSES = new Map<unknown, string>([
	"resources", "resource-topology", "resource-metrics", "resource-details",
	"resource-yaml", "resource-events", "kube-contexts", "kube-namespaces",
	"kube-resource-kinds", "kube-present-custom-resource-kinds", "kubeconfig-sources",
	"deployment-revisions", "argo-connection-status", "argo-server-discovery", "argo-detect",
	"argo-apps", "argo-appsets", "argo-appprojects", "argo-app-details", "argo-appset-details",
	"argo-appproject-details", "argo-workspace", "flux-detect", "flux-resources", "flux-resource-details",
	"helm-releases", "helm-release-details", "helm-release-reconciliation", "rbac-inspection",
	"incident-cockpit", "port-forwards", "pod-exec-sessions", "app-usage-metrics",
	"backend-diagnostics", "backend-cache-diagnostics",
].map((root) => [root, root]));

interface QueryMemoryRoot {
	root: string;
	queries: number;
	active: number;
	observed: number;
	inactive: number;
	unobserved: number;
	items: number;
	jsonBytes: number | null;
	unavailablePayloads: number;
}

function payloadItems(query: Query): number {
	const data = query.state.data;
	if (Array.isArray(data)) return data.length;
	if (data === undefined || data === null) return 0;
	if (!(data instanceof Object)) return 1;
	let items = 0;
	let collection = false;
	for (const [key, value] of Object.entries(data)) {
		if (["nodes", "edges", "pods", "workloads", "warnings"].includes(key) && Array.isArray(value)) {
			collection = true;
			items += value.length;
		}
	}
	return collection ? items : 1;
}

export function collectQueryMemory(client: QueryClient, includePayloadEstimates = true) {
	const started = performance.now();
	const roots = new Map<string, QueryMemoryRoot>();
	for (const query of client.getQueryCache().getAll()) {
		const root = ROOT_CLASSES.get(query.queryKey[0]) ?? "other";
		const row = roots.get(root) ?? {
			root, queries: 0, active: 0, observed: 0, inactive: 0, unobserved: 0,
			items: 0, jsonBytes: includePayloadEstimates ? 0 : null, unavailablePayloads: 0,
		};
		row.queries += 1;
		row.active += Number(query.isActive());
		row.inactive += Number(!query.isActive());
		row.observed += Number(query.getObserversCount() > 0);
		row.unobserved += Number(query.getObserversCount() === 0);
		row.items += payloadItems(query);
		if (includePayloadEstimates && query.state.data !== undefined) {
			try {
				const serialized = JSON.stringify(query.state.data);
				if (serialized === undefined) throw new Error("Payload is not JSON serializable");
				if (row.jsonBytes !== null) row.jsonBytes += new TextEncoder().encode(serialized).byteLength;
			} catch {
				row.jsonBytes = null;
				row.unavailablePayloads += 1;
			}
		}
		roots.set(root, row);
	}
	return {
		measurementMs: performance.now() - started,
		payloadEstimate: includePayloadEstimates ? "utf8-json-bytes-not-heap-size" : "not-collected",
		roots: [...roots.values()].sort((a, b) => a.root.localeCompare(b.root)),
	};
}

declare global {
	interface Window {
		__KUBECOVE_MEMORY_PROFILE__?: (includePayloadEstimates?: boolean) => ReturnType<typeof collectQueryMemory>;
	}
}

export function attachMemoryProfile(client: QueryClient): void {
	if (process.env.KUBECOVE_PUBLIC_PROFILE !== "true") return;
	window.__KUBECOVE_MEMORY_PROFILE__ = (includePayloadEstimates) => collectQueryMemory(client, includePayloadEstimates);
}

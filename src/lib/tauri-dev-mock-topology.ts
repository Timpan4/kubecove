import { networkIngressTargets, networkServiceFallbackSelectors, networkServicePodMap, now, ownershipDependencyHints, res, resources } from "./tauri-dev-mock-data";
import type { ResourceSummary, ResourceTopology } from "./types";

// Browser-mock ownership and network-flow maps, built from the mock rows of one cluster.

// payments-api is at revision 3, and its revision 3 ReplicaSet owns the Pod. Its revision 2
// ReplicaSet has no Pods, so "Hide historical ReplicaSets" removes it from the map.
const PAYMENTS_API_REVISION = 3;
const PAYMENTS_API_HISTORICAL_REVISION = 2;
const PAYMENTS_API_HISTORICAL_REPLICASET = "payments-api-6b5f8c9d7";

export function ownershipTopology(cluster: string, namespaces: ReadonlySet<string>, rows: ResourceSummary[]): ResourceTopology {
	const deploymentRows = rows.filter((row) => row.kind === "Deployment" && inNamespaceScope(row, namespaces));
	const replicaSets = deploymentRows.map((row) => replicaSetFor(row, cluster));
	const historicalRows = deploymentRows.filter(hasHistoricalReplicaSet);
	const historicalReplicaSets = historicalRows.map((row) => historicalReplicaSetFor(row, cluster));
	const podRows = rows.filter((row) => row.kind === "Pod" && inNamespaceScope(row, namespaces));
	const standaloneRows = rows.filter((row) => isOwnershipStandalone(row) && inNamespaceScope(row, namespaces) && !isOwnershipNoise(row));
	const nodes = [
		...deploymentRows.map((row) => topologyNode(row, cluster, [], deploymentRevisionOf(row))),
		...replicaSets,
		...historicalReplicaSets,
		...podRows.map((row) => topologyNode(row, cluster)),
		...standaloneRows.map((row) => topologyNode(row, cluster)),
	];
	const edges = [
		...deploymentRows.map((row) => topologyEdge(topologyId(row, cluster), replicaSetId(row, cluster), "owns")),
		...historicalRows.map((row) => topologyEdge(topologyId(row, cluster), historicalReplicaSetId(row, cluster), "owns")),
		...podRows.flatMap((row) => {
			const owner = deploymentRows.find((deployment) => deployment.namespace === row.namespace && row.name.startsWith(`${deployment.name}-`));
			return owner ? [topologyEdge(replicaSetId(owner, cluster), topologyId(row, cluster), "owns")] : [];
		}),
		...rows
			.filter((row) => inNamespaceScope(row, namespaces))
			.flatMap((resource) =>
				ownershipDependencyHints
					.filter((hint) =>
						hint.resourceKind === resource.kind &&
						hint.resourceName === resource.name &&
						hint.resourceNamespace === resource.namespace,
					)
					.map((hint) => {
						const owner = rows.find(
							(ownerRow) =>
								ownerRow.kind === hint.ownerKind &&
								ownerRow.name === hint.ownerName &&
								ownerRow.namespace === hint.ownerNamespace &&
								inNamespaceScope(ownerRow, namespaces),
						);
						if (!owner) return null;
						return topologyEdge(
							topologyId(owner, cluster),
							topologyId(resource, cluster),
							"owns",
						);
					})
					.filter((edge): edge is NonNullable<typeof edge> => edge !== null),
		),
	].filter((edge) => nodes.some((node) => node.id === edge.source) && nodes.some((node) => node.id === edge.target));
	return { nodes, edges, warnings: ["Browser mock ownership map includes curated support-resource ownership."] };
}

export function networkTopology(cluster: string, namespaces: ReadonlySet<string>, allRows: ResourceSummary[]): ResourceTopology {
	const rows = allRows.filter(
		(row) => ["Ingress", "Service", "Pod"].includes(row.kind) && inNamespaceScope(row, namespaces),
	);
	const services = rows.filter((row) => row.kind === "Service");
	const ingresses = rows.filter((row) => row.kind === "Ingress");
	const pods = rows.filter((row) => row.kind === "Pod");
	const endpointSlices = services.flatMap((service) => {
		const key = serviceNamespaceKey(service);
		const mappedPodNames = new Set(networkServicePodMap[key] ?? []);
		const matchingPods = pods.filter((pod) => pod.namespace === service.namespace && mappedPodNames.has(pod.name));
		return matchingPods.length === 0 ? [] : [endpointSliceNode(service.name, service.namespace ?? "", cluster, matchingPods.map((pod) => pod.name))];
	});
	const nodes = [
		...rows.map((row) => topologyNode(row, cluster, networkPortHints(row))),
		...endpointSlices,
	];
	const edges = [
		...networkIngressTargets.map((link) => {
				const ingress = ingresses.find((row) =>
					row.kind === "Ingress" &&
					row.namespace === link.namespace &&
					row.name === link.ingress,
				);
				const service = services.find((row) =>
					row.kind === "Service" &&
					row.namespace === link.namespace &&
					row.name === link.service,
				);
				if (!ingress || !service) return null;
				return topologyEdge(topologyId(ingress, cluster), topologyId(service, cluster), "routesTo");
			})
			.filter((edge): edge is Exclude<ReturnType<typeof topologyEdge>, null> => edge !== null),
		...services.flatMap((service) => {
			const key = serviceNamespaceKey(service);
			const configuredPodNames = networkServicePodMap[key] ?? [];
			const configuredPods = pods.filter((pod) => configuredPodNames.includes(pod.name) && pod.namespace === service.namespace);
			if (configuredPods.length > 0) {
				const sliceSummary = endpointSliceSummary(service.name, service.namespace ?? "", cluster, configuredPods.map((pod) => pod.name));
				const fallbackEdges = [
					topologyEdge(topologyId(service, cluster), topologyId(sliceSummary, cluster), "targets"),
				];
				return fallbackEdges.concat(
					configuredPods.map((pod) => topologyEdge(topologyId(sliceSummary, cluster), topologyId(pod, cluster), "targets")),
				);
			}

			const selectorHint = networkServiceFallbackSelectors[key];
			const selectorMode = selectorHint?.split("=") ?? [];
			const selectorPodNames = selectorMode.length === 2
				? pods
					.filter((pod) => pod.namespace === service.namespace && pod.name.startsWith(`${selectorMode[1]}-`))
					.map((pod) => pod.name)
				: pods
					.filter((pod) => pod.name.startsWith(`${service.name}-`))
					.map((pod) => pod.name);

			const uniqueSelectorPods = [...new Set(selectorPodNames)];
			return uniqueSelectorPods.map((name) =>
				topologyEdge(topologyId(service, cluster), topologyId(resourceRef("Pod", name, service.namespace ?? ""), cluster), "selects"),
			);
		}),
	].filter((edge) => nodes.some((node) => node.id === edge.source) && nodes.some((node) => node.id === edge.target));
	return { nodes, edges, warnings: ["Browser mock network flow uses curated Service, Ingress, and EndpointSlice relationships."] };
}

function namespaceKeyOf(namespace: string | null, name: string): string {
	return `${namespace ?? ""}:${name}`;
}

function serviceNamespaceKey(service: Pick<ResourceSummary, "namespace" | "name">): string {
	return namespaceKeyOf(service.namespace, service.name);
}

function endpointSliceSummary(serviceName: string, namespace: string, cluster: string, pods: string[] = []): ResourceSummary {
	return {
		...res("EndpointSlice", `${serviceName}-mock`, namespace, "healthy", undefined, "Ready", 0, "platform", {
			apiVersion: "discovery.k8s.io/v1",
			cluster,
		}),
		age: "12d",
		status: "Ready",
		metrics: {
			kind: "EndpointSlice",
			cluster,
			name: `${serviceName}-mock`,
			namespace,
			cpuMillicores: 20,
			memoryBytes: 14_000_000,
			sampledAt: now,
			sourcePods: pods,
		},
	};
}

function isOwnershipStandalone(row: ResourceSummary): boolean {
	return ["Service", "Ingress", "ConfigMap", "Secret", "PersistentVolumeClaim", "PersistentVolume", "StorageClass"].includes(row.kind);
}

function isOwnershipNoise(row: ResourceSummary): boolean {
	return row.kind === "ConfigMap" && row.name === "kube-root-ca.crt";
}

function topologyNode(row: ResourceSummary, cluster: string, portHints: string[] = [], deploymentRevision?: number) {
	return {
		id: topologyId(row, cluster),
		kind: row.kind,
		name: row.name,
		namespace: row.namespace,
		status: row.status,
		health: row.health,
		portHints,
		selectable: true,
		...(deploymentRevision === undefined ? {} : { deploymentRevision }),
		summary: { ...row, cluster },
	};
}

function deploymentRevisionOf(row: ResourceSummary): number | undefined {
	return row.name === "payments-api" ? PAYMENTS_API_REVISION : undefined;
}

function hasHistoricalReplicaSet(row: ResourceSummary): boolean {
	return row.name === "payments-api";
}

function replicaSetFor(row: ResourceSummary, cluster: string) {
	const name = row.name === "payments-api" ? "payments-api-7d9c9b7f8d" : `${row.name}-54b7f7bdbc`;
	const summary = res("ReplicaSet", name, row.namespace, row.health, row.ready, row.status, 0, row.argoApp ? "shop" : "platform", { apiVersion: "apps/v1", cluster });
	return topologyNode(summary, cluster, [], deploymentRevisionOf(row));
}

function historicalReplicaSetFor(row: ResourceSummary, cluster: string) {
	const summary = res("ReplicaSet", PAYMENTS_API_HISTORICAL_REPLICASET, row.namespace, "healthy", "0/0", "Scaled to 0", 0, row.argoApp ? "shop" : "platform", { apiVersion: "apps/v1", cluster });
	return topologyNode(summary, cluster, [], PAYMENTS_API_HISTORICAL_REVISION);
}

function historicalReplicaSetId(row: ResourceSummary, cluster: string): string {
	return topologyId({ apiVersion: "apps/v1", kind: "ReplicaSet", namespace: row.namespace, name: PAYMENTS_API_HISTORICAL_REPLICASET }, cluster);
}

function topologyId(row: Pick<ResourceSummary, "apiVersion" | "kind" | "namespace" | "name">, cluster: string): string {
	return `${cluster}:${row.apiVersion ?? "v1"}:${row.kind}:${row.namespace ?? ""}:${row.name}`;
}

function replicaSetId(row: ResourceSummary, cluster: string): string {
	const name = row.name === "payments-api" ? "payments-api-7d9c9b7f8d" : `${row.name}-54b7f7bdbc`;
	return topologyId({ apiVersion: "apps/v1", kind: "ReplicaSet", namespace: row.namespace, name }, cluster);
}

function endpointSliceNode(
	serviceName: string,
	namespace: string,
	cluster: string,
	pods: string[] = [],
): ReturnType<typeof topologyNode> {
	const summary = endpointSliceSummary(serviceName, namespace, cluster, pods);
	return { ...topologyNode(summary, cluster, networkPortHints(summary)), selectable: false };
}

function topologyEdge(source: string, target: string, relation: ResourceTopology["edges"][number]["relation"]) {
	return { id: `${source}->${target}`, source, target, relation };
}

function resourceRef(kindName: string, name: string, namespace: string): ResourceSummary {
	return resources.find((row) => row.kind === kindName && row.name === name && row.namespace === namespace) ?? res(kindName, name, namespace, "unknown");
}

function inNamespaceScope(row: Pick<ResourceSummary, "namespace"> | undefined, namespaces: ReadonlySet<string>): row is ResourceSummary {
	if (!row) return false;
	return namespaces.size === 0 || (row.namespace !== null && namespaces.has(row.namespace));
}

function networkPortHints(row: ResourceSummary): string[] {
	if (row.kind === "Ingress") return ["https:443->payments-api:80"];
	if (row.kind === "Service" && row.name === "payments-api") return ["http:80->8080", "metrics:9090->9090"];
	if (row.kind === "Service") return ["http:80->8080"];
	if (row.kind === "Pod") return ["app:8080", "metrics:9090"];
	return [];
}

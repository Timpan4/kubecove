// Exports the browser-mock command responses as one JSON fixture for the GPUI
// visual driver, so both frontends render the same data.
// Usage: bun e2e/parity/visual/export-fixture.ts <output.json>
import { writeFile } from "node:fs/promises";
import { createDevMockTauriClient } from "../../../src/lib/tauri-dev-mocks";
import type { ClusterContext, DiscoveredResourceKind, NamespaceSummary, ResourceDetailsFull, ResourceSummary, ResourceTopology } from "../../../src/lib/types";

// Browser mocks omit owner fields the Rust model requires; fill them with the
// values the web frontend infers (`owner.partial ?? confidence !== "inventory"`).
function rustShape(row: ResourceSummary): ResourceSummary {
	const owner = row.gitOpsOwner;
	return owner ? { ...row, gitOpsOwner: { provenance: "browser-mock", ...owner, partial: owner.partial ?? owner.confidence !== "inventory" } } : row;
}

const output = process.argv[2];
if (!output) throw new Error("Usage: export-fixture.ts <output.json>");
const client = createDevMockTauriClient();
const contexts = await client.invoke<ClusterContext[]>("list_kube_contexts");
const clusterContext = contexts.find((context) => context.isCurrent)?.name ?? contexts[0]?.name;
const namespaces = await client.invoke<NamespaceSummary[]>("list_namespaces", { clusterContext });
const kinds = await client.invoke<DiscoveredResourceKind[]>("list_resource_kinds", { clusterContext });
const resources = await client.invoke<ResourceSummary[]>("list_resources", { clusterContext });
const details: ResourceDetailsFull[] = [];
for (const row of resources) {
	const detail = await client.invoke<ResourceDetailsFull>("get_resource_details", { clusterContext, kind: row.kind, name: row.name, namespace: row.namespace });
	details.push({ ...detail, summary: rustShape(detail.summary) });
}
// The web map reads every namespace (`namespaces: []`) for the All scope the views capture.
const topology = await client.invoke<ResourceTopology>("list_resource_topology", { clusterContext, namespaces: [], mode: "ownership" });
const rustTopology = { ...topology, nodes: topology.nodes.map((node) => ({ ...node, summary: rustShape(node.summary) })) };
await writeFile(output, JSON.stringify({ contexts, namespaces, kinds, resources: resources.map(rustShape), details, topology: rustTopology }, null, "\t"));
console.log(`fixture: ${resources.length} resources, ${details.length} details -> ${output}`);

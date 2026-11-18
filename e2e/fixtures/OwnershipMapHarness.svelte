<script lang="ts">
	import OwnershipMap from "@/features/resources/OwnershipMap.svelte";
	import type { ResourceTopology } from "@/lib/types";
	const { initialTopology, contexts }: { initialTopology: ResourceTopology; contexts: string[] } = $props();
	// svelte-ignore state_referenced_locally -- The fixture replaces it through refresh().
	let topology = $state.raw(initialTopology);
	let error = $state<Error | null>(null);
	let selectedNodeId = $state<string | null>(null);
	export function select(nodeId: string | null) { selectedNodeId = nodeId; }
	export function refresh(next: ResourceTopology) {
		topology = next;
		error = null;
	}
	export function failRefresh() {
		const workloads = initialTopology.nodes.filter((node) => node.kind === "Deployment");
		error = new Error([
			"Topology request unavailable",
			`https://example.invalid/apis/apps/v1/deployments/${workloads.map((node) => node.name).join("-")}`,
			...workloads.map((node) => `Unable to read ownership for ${node.namespace}/${node.name}`),
		].join("\n"));
	}
</script>

<div class="dark fixed inset-0 z-10 flex h-screen bg-background p-4 text-foreground" data-ownership-map-harness>
	<OwnershipMap
		{topology}
		isLoading={false}
		isError={false}
		{error}
		mode="ownership"
		{selectedNodeId}
		showFullTopologyOnSelection
		fitViewKey="ownership-map-harness"
		{contexts}
		mapContext={contexts[0] ?? ""}
		onModeChange={() => {}}
		onNodeSelect={(nodeId) => (selectedNodeId = nodeId)}
	/>
</div>

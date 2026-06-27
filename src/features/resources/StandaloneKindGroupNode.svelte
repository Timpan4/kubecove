<script lang="ts">
	import { ChevronDown, ChevronRight } from "lucide-svelte";
	import type { NodeProps } from "@xyflow/svelte";
	import { Badge } from "@/components/ui/svelte";
	import { getResourceKindVisual } from "@/app/svelte/resourceVisuals";
	import { cnfast } from "@/lib/utils";
	import type { FlowTopologyNode, FlowTopologyNodeData } from "./topologyModel";

	let { data }: NodeProps<FlowTopologyNode> = $props();

	const nodeData = $derived(data as FlowTopologyNodeData);
	const visual = $derived(getResourceKindVisual(nodeData.kind ?? "Resource"));
	const Icon = $derived(visual.icon);
	const ToggleIcon = $derived(nodeData.expanded ? ChevronDown : ChevronRight);
	const isSelected = $derived(Boolean(nodeData.selected));
</script>

<div
	class={cnfast(
		"resource-topology-node relative h-full w-full rounded-md border border-dashed border-[var(--topology-node-border)] px-3 py-2 text-xs shadow-sm transition-colors hover:bg-accent/20",
		visual.surfaceClassName,
		isSelected && "resource-topology-node-selected ring-2 ring-primary",
		nodeData.dimmed && "opacity-35",
	)}
	data-selected={isSelected ? "true" : undefined}
>
	<span
		class="pointer-events-none absolute -top-4 left-1/2 h-4 border-l border-dashed border-muted-foreground/35"
		aria-hidden="true"
	></span>
	<div class="flex h-full min-w-0 items-center justify-between gap-3">
		<div class="flex min-w-0 items-center gap-2">
			<ToggleIcon class="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
			<Icon class={cnfast("size-3.5 shrink-0", visual.className)} aria-hidden="true" />
			<span class="min-w-0 truncate font-semibold text-foreground" title={`Ownerless ${nodeData.kind}`}>
				Ownerless {nodeData.kind}
			</span>
		</div>
		<Badge variant="outline" class="h-5 rounded-full px-2 text-[0.625rem]">
			{nodeData.count}
		</Badge>
	</div>
</div>

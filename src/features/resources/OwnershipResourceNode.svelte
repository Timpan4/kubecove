<script lang="ts">
	import { Handle, Position, type NodeProps } from "@xyflow/svelte";
	import { Badge } from "@/components/ui/svelte";
	import { getResourceKindVisual } from "@/app/svelte/resourceVisuals";
	import { formatCompactResourceMetrics } from "@/lib/resource-metrics";
	import { cnfast } from "@/lib/utils";
	import { smartKubernetesName } from "./ownership-node-name";
	import {
		topologyRailTone,
		topologyReadyText,
		topologyReadyTone,
		topologyRestartTone,
		topologyStatusTone,
		type FlowTopologyNode,
		type FlowTopologyNodeData,
		type TopologyStoplightTone,
	} from "./topologyModel";

	let {
		data,
		selected,
		targetPosition = Position.Left,
		sourcePosition = Position.Right,
	}: NodeProps<FlowTopologyNode> = $props();

	const nodeData = $derived(data as FlowTopologyNodeData);
	const node = $derived(nodeData.node);
	const visual = $derived(getResourceKindVisual(node?.kind ?? nodeData.kind ?? "Resource"));
	const Icon = $derived(visual.icon);
	const displayName = $derived(
		node ? smartKubernetesName(node.name, node.kind) : nodeData.label,
	);
	const kindLabel = $derived((node?.kind ?? nodeData.kind)?.toUpperCase());
	const scopeText = $derived(node?.namespace ?? "Cluster scoped");
	const age = $derived(node?.summary.age);
	const metricHint = $derived(
		node ? formatCompactResourceMetrics(node.metrics ?? node.summary.metrics) : undefined,
	);
	const portHint = $derived(
		nodeData.showPortHints && node?.portHints?.length
			? node.portHints.slice(0, 3).join(", ")
			: undefined,
	);
	const lifecycleStatus = $derived(node?.status || node?.health);
	const statusText = $derived(portHint || lifecycleStatus || "No status");
	const readyText = $derived(topologyReadyText(node?.summary.ready, lifecycleStatus));
	const restartCount = $derived(node?.summary.restarts ?? 0);
	const restartText = $derived(
		restartCount > 0
			? `${restartCount} ${restartCount === 1 ? "restart" : "restarts"}`
			: undefined,
	);
	const metaText = $derived(
		[scopeText, age, metricHint].filter((value): value is string => Boolean(value)).join(" · "),
	);
	const statusTone = $derived(topologyStatusTone(lifecycleStatus));
	const readyTone = $derived(topologyReadyTone(node?.summary.ready, lifecycleStatus));
	const restartTone = $derived(topologyRestartTone(restartCount));
	const railTone = $derived(
		topologyRailTone(lifecycleStatus, node?.summary.ready, restartCount, node?.health),
	);
	const statusTitle = $derived(
		[statusText, readyText, metaText, restartText]
			.filter((value): value is string => Boolean(value))
			.join(" · "),
	);
	const isSelected = $derived(Boolean(selected || nodeData.selected));

	function stoplightClass(tone: TopologyStoplightTone): string {
		return `resource-topology-stoplight-${tone}`;
	}

	function chipClass(tone: TopologyStoplightTone, extra = ""): string {
		return cnfast("resource-topology-status-chip", `resource-topology-chip-${tone}`, extra);
	}
</script>

<div class="relative h-full w-full overflow-visible" data-selected={isSelected ? "true" : undefined}>
	<Handle
		type="target"
		position={targetPosition}
		isConnectable={false}
		style="opacity: 0"
	/>
	<Handle
		type="source"
		position={sourcePosition}
		isConnectable={false}
		style="opacity: 0"
	/>
	<div
		class={cnfast(
			"resource-topology-node relative flex h-full min-h-[90px] w-full flex-col overflow-hidden rounded-md border px-3.5 py-3 pl-5 text-left text-xs transition-opacity",
			"svelte-ownership-resource-node bg-card text-card-foreground",
			stoplightClass(railTone),
			visual.surfaceClassName,
			nodeData.connected && "resource-topology-node-connected ring-1 ring-primary/30",
			isSelected && "resource-topology-node-selected ring-2 ring-primary",
			nodeData.dimmed && "opacity-35",
		)}
	>
	<div class="grid min-w-0 grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-x-2">
		<Icon class={cnfast("size-3 shrink-0", visual.className)} aria-hidden="true" />
		<span
			class="min-w-0 truncate text-sm font-semibold leading-none text-foreground"
			title={node ? `${node.kind}/${node.name}` : displayName}
		>
			{displayName}
		</span>
		{#if node && kindLabel}
			<Badge
				variant="outline"
				class={cnfast(
					"h-5 max-w-[5.75rem] shrink-0 rounded-sm px-2 text-[0.625rem] font-semibold uppercase leading-none tracking-[0.04em]",
					visual.badgeClassName,
				)}
				title={node.kind}
			>
				<span class="truncate">{kindLabel}</span>
			</Badge>
		{/if}
	</div>

	{#if node && metaText}
		<div
			class="mt-1.5 min-w-0 truncate pl-6 text-xs leading-none text-muted-foreground"
			title={metaText}
		>
			{metaText}
		</div>
	{/if}

	<div
		class="mt-auto flex min-w-0 items-center gap-1.5 border-t border-border/35 pt-1.5 text-xs leading-none"
		title={statusTitle}
	>
		<span
			class={cnfast(
				"resource-topology-status-dot size-2 shrink-0 rounded-full",
				`resource-topology-chip-${statusTone}`,
			)}
		></span>
		<span class={chipClass(statusTone)}>
			<span class="truncate">{statusText}</span>
		</span>
		{#if readyText}
			<span class={chipClass(readyTone)}>
				<span class="truncate">{readyText}</span>
			</span>
		{/if}
		{#if restartText}
			<span class={chipClass(restartTone, "ml-auto")}>
				<span class="truncate">{restartText}</span>
			</span>
		{/if}
	</div>
	</div>
</div>

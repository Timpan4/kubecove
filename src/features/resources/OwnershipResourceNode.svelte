<script lang="ts">
	import { Handle, Position, type NodeProps } from "@xyflow/svelte";
	import { Badge } from "@/components/ui/svelte";
	import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
	import { formatExactTimestamp } from "@/components/timestamp-format";
	import { getResourceKindVisual } from "@/app/svelte/resourceVisuals";
	import { formatCompactResourceMetrics } from "@/lib/resource-metrics";
	import { cn } from "@/lib/utils";
	import { smartKubernetesName } from "./ownership-node-name";
	import type { FlowTopologyNode, FlowTopologyNodeData } from "./topologyModel";

	type BadgeTone = keyof typeof STATUS_BADGE_STYLES;

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
	const scopeText = $derived(node?.namespace ?? "cluster");
	const age = $derived(node?.summary.age);
	const exactAge = $derived(
		formatExactTimestamp(node?.summary.createdAt, "local", "second"),
	);
	const ageTitle = $derived(exactAge ? `Created ${exactAge}` : age);
	const metricHint = $derived(
		node ? formatCompactResourceMetrics(node.metrics ?? node.summary.metrics) : undefined,
	);
	const primaryStatus = $derived(
		nodeData.showPortHints && node?.portHints?.length
			? node.portHints.slice(0, 3).join(", ")
			: node?.status,
	);
	const restartCount = $derived(node?.summary.restarts ?? 0);
	const healthStyle = $derived(STATUS_BADGE_STYLES[healthTone(node?.health)]);
	const isSelected = $derived(Boolean(selected || nodeData.selected));

	function healthTone(health: string | undefined): BadgeTone {
		if (health === "healthy") return "success";
		if (health === "attention") return "warning";
		if (health === "degraded") return "error";
		if (health === "restarted") return "info";
		return "neutral";
	}
</script>

<div
	class={cn(
		"resource-topology-node relative grid h-full min-h-[104px] w-full grid-rows-[20px_20px_20px] content-center gap-2.5 overflow-hidden rounded-md border px-4 py-2.5 text-xs shadow-sm transition-opacity",
		"svelte-ownership-resource-node border-2 border-l-[5px] border-[var(--topology-node-border)] bg-card text-card-foreground",
		visual.surfaceClassName,
		node?.health === "degraded" && "resource-topology-node-health-degraded",
		node?.health === "attention" && "resource-topology-node-health-attention",
		node?.health === "restarted" && "resource-topology-node-health-restarted",
		nodeData.connected && "resource-topology-node-connected ring-1 ring-primary/30",
		isSelected && "resource-topology-node-selected ring-2 ring-primary",
		nodeData.dimmed && "opacity-35",
	)}
	data-selected={isSelected ? "true" : undefined}
>
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

	<div class="grid min-w-0 grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-2">
		<Icon class={cn("size-4 shrink-0", visual.className)} aria-hidden="true" />
		<span class="min-w-0 truncate text-[0.9rem] font-bold leading-tight" title={node?.name ?? displayName}>
			{displayName}
		</span>
		{#if node?.health}
			<Badge
				variant={healthStyle.variant}
				class={cn("h-5 shrink-0 rounded-sm px-2 text-[0.625rem] font-semibold", healthStyle.className)}
			>
				{node.health}
			</Badge>
		{/if}
	</div>

	<div class="flex min-w-0 items-center gap-2 whitespace-nowrap text-[0.6875rem] text-muted-foreground">
		{#if node}
			<Badge
				variant="outline"
				class={cn(
					"h-5 max-w-[7.5rem] shrink-0 rounded-sm px-2 text-[0.625rem] font-semibold",
					visual.badgeClassName,
				)}
				title={node.kind}
			>
				<span class="truncate">{node.kind}</span>
			</Badge>
			<Badge
				variant="outline"
				class="h-5 max-w-[6rem] shrink-0 rounded-sm border-border/55 bg-muted/25 px-2 text-[0.625rem] font-medium text-muted-foreground"
				title={scopeText}
			>
				<span class="truncate">{scopeText}</span>
			</Badge>
			{#if age}
				<Badge
					variant="outline"
					class="h-5 shrink-0 rounded-sm border-border/55 bg-muted/25 px-2 text-[0.625rem] font-medium text-muted-foreground"
					title={ageTitle}
				>
					{age}
				</Badge>
			{/if}
		{/if}
	</div>

	<div class="flex min-w-0 items-center justify-between gap-3">
		<div class="flex min-w-0 items-center gap-2">
			{#if primaryStatus}
				<Badge
					variant="outline"
					class="h-5 max-w-[8.5rem] rounded-sm border-primary/45 bg-primary/10 px-2 text-[0.625rem] font-semibold"
					title={primaryStatus}
				>
					<span class="truncate">{primaryStatus}</span>
				</Badge>
			{/if}
			{#if node?.summary.ready}
				<Badge
					variant="outline"
					class="h-5 rounded-sm border-emerald-500/30 bg-emerald-500/10 px-2 text-[0.625rem] font-semibold text-emerald-300"
					title={node.summary.ready}
				>
					<span class="truncate">{node.summary.ready}</span>
				</Badge>
			{/if}
			{#if metricHint}
				<Badge
					variant="outline"
					class="h-5 max-w-[7rem] rounded-sm px-2 text-[0.625rem] text-muted-foreground"
					title={metricHint}
				>
					<span class="truncate">{metricHint}</span>
				</Badge>
			{/if}
		</div>
		<div class="flex shrink-0 items-center gap-2">
			{#if restartCount > 0}
				<Badge
					variant="outline"
					class="h-5 rounded-sm border-red-400/35 bg-red-500/10 px-2 text-[0.625rem] font-semibold text-red-300"
				>
					{restartCount} restarts
				</Badge>
			{/if}
		</div>
	</div>
</div>

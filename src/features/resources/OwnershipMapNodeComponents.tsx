import {
	Handle,
	Position,
	type NodeProps,
} from "@xyflow/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { STATUS_BADGE_STYLES } from "@/components/status-badge-styles";
import { Badge } from "@/components/ui/badge";
import type { TopologyHealth } from "@/lib/types";
import { formatCompactResourceMetrics } from "@/lib/resource-metrics";
import { getResourceKindVisual } from "@/lib/resource-visuals";
import { cn } from "@/lib/utils";
import { smartKubernetesName } from "./ownership-node-name";
import {
	topologyNodeClassName,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
} from "./topology";

function topologyHealthBadgeStyle(health: TopologyHealth) {
	switch (health) {
		case "healthy":
			return STATUS_BADGE_STYLES.success;
		case "attention":
			return STATUS_BADGE_STYLES.warning;
		case "degraded":
			return STATUS_BADGE_STYLES.error;
		case "restarted":
			return STATUS_BADGE_STYLES.info;
		default:
			return STATUS_BADGE_STYLES.neutral;
	}
}

export function HealthBadge({ health }: { health: TopologyHealth }) {
	const badgeStyle = topologyHealthBadgeStyle(health);

	return (
		<Badge
			variant={badgeStyle.variant}
			className={cn(
				"h-[1.125rem] max-w-[4.75rem] shrink-0 rounded-sm px-1.5 text-[0.625rem] leading-none",
				badgeStyle.className,
			)}
		>
			<span className="truncate">{health}</span>
		</Badge>
	);
}

export function ValueBadge({
	value,
	className,
}: {
	value: string;
	className?: string;
}) {
	return (
		<Badge
			variant="outline"
			className={cn(
				"h-[1.125rem] min-w-0 max-w-full justify-start rounded-sm px-1.5 text-[0.625rem] font-medium leading-none",
				className,
			)}
			title={value}
		>
			<span className="truncate">{value}</span>
		</Badge>
	);
}

export function KindBadge({
	kind,
	className,
}: {
	kind: string;
	className?: string;
}) {
	return (
		<Badge
			variant="outline"
			className={cn(
				"h-4 max-w-[5.75rem] shrink-0 rounded-sm px-1.5 text-[0.625rem] font-semibold leading-none shadow-none",
				className,
			)}
			title={kind}
		>
			<span className="truncate">{kind}</span>
		</Badge>
	);
}

export function TopologyText({
	content,
	title = content,
	className,
}: {
	content: string;
	title?: string;
	className?: string;
}) {
	return (
		<span className={cn("block min-w-0 truncate", className)} title={title}>
			{content}
		</span>
	);
}

export function OwnershipResourceNode({
	data,
	selected,
}: NodeProps<OwnershipResourceGraphNode>) {
	const node = data.node;
	const visual = getResourceKindVisual(node.kind);
	const Icon = visual.icon;
	const selectedOrConnected = data.selected || data.connected;
	const scopeText = node.namespace ?? "Cluster scoped";
	const displayName = smartKubernetesName(node.name, node.kind);
	const age = node.summary.age;
	const portHint = data.showPortHints
		? node.portHints?.slice(0, 3).join(", ")
		: undefined;
	const metricHint = formatCompactResourceMetrics(node.metrics ?? node.summary.metrics);
	const primaryValue = [portHint || node.status, metricHint]
		.filter((value): value is string => Boolean(value))
		.join(" · ");

	return (
		<div
			className={cn(
				topologyNodeClassName(
					node,
					data.selected || selected ? node.id : null,
					node.id,
					selectedOrConnected,
					visual.surfaceClassName,
				),
				"relative grid h-[78px] w-full grid-rows-[auto_auto_auto] content-start gap-1.5 px-2.5 py-2",
				data.dimmed && "opacity-35",
			)}
			data-selected={data.selected ? "true" : undefined}
		>
			<Handle
				type="target"
				position={Position.Left}
				isConnectable={false}
				style={{ opacity: 0 }}
			/>
			<Handle
				type="source"
				position={Position.Right}
				isConnectable={false}
				style={{ opacity: 0 }}
			/>
			<div className="grid min-w-0 grid-cols-[0.875rem_minmax(0,1fr)_auto] items-center gap-x-2">
				<Icon className={cn("size-3.5 shrink-0", visual.className)} />
				<TopologyText
					content={displayName}
					title={`${node.kind}/${node.name}`}
					className="text-[0.6875rem] font-semibold leading-none text-foreground"
				/>
				<HealthBadge health={node.health} />
			</div>
			<div className="flex min-w-0 items-center gap-1.5">
				<KindBadge kind={node.kind} className={visual.badgeClassName} />
				<TopologyText
					content={scopeText}
					className="text-[0.64rem] leading-tight text-muted-foreground"
				/>
			</div>
			<div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
				{primaryValue ? (
					<ValueBadge
						value={primaryValue}
						className="border-primary/45 bg-primary/10"
					/>
				) : (
					<span className="min-w-0" />
				)}
				<ValueBadge
					value={age}
					className="border-muted-foreground/25 bg-muted-foreground/10 text-muted-foreground"
				/>
			</div>
		</div>
	);
}

export function StandaloneKindGroupNode({
	data,
}: NodeProps<StandaloneKindGroupGraphNode>) {
	const visual = getResourceKindVisual(data.kind);
	const Icon = visual.icon;
	const ToggleIcon = data.expanded ? ChevronDown : ChevronRight;

	return (
		<div
			className={cn(
				"resource-topology-node h-full w-full rounded-md border border-border/80 shadow-sm transition-colors hover:bg-accent/20",
				visual.surfaceClassName,
				data.dimmed && "opacity-35",
			)}
		>
			<div
				className={cn(
					"flex h-full items-center justify-between gap-3 px-3",
					data.expanded && "h-[38px] border-b border-border/70",
				)}
			>
				<div className="flex min-w-0 items-center gap-2">
					<ToggleIcon className="size-3 shrink-0 text-muted-foreground" />
					<Icon className={cn("size-3.5 shrink-0", visual.className)} />
					<TopologyText
						content={`Ownerless ${data.kind}`}
						className="text-[0.6875rem] font-semibold text-foreground"
					/>
				</div>
				<Badge variant="outline" className="h-4 rounded-sm px-1.5 text-[0.625rem]">
					{data.count}
				</Badge>
			</div>
		</div>
	);
}

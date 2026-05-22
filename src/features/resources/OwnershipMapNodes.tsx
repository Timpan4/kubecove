import {
	Handle,
	Position,
	type NodeProps,
	type NodeTypes,
} from "@xyflow/react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { getResourceKindVisual } from "@/lib/resource-visuals";
import { cn } from "@/lib/utils";
import {
	topologyNodeClassName,
	type OwnershipResourceGraphNode,
	type StandaloneKindGroupGraphNode,
} from "./topology";

function HealthBadge({ health }: { health: string }) {
	return (
		<Badge
			variant={health === "degraded" ? "destructive" : "outline"}
			className="h-[1.125rem] max-w-[4.75rem] shrink-0 rounded-sm px-1.5 text-[0.625rem] leading-none"
		>
			<span className="truncate">{health}</span>
		</Badge>
	);
}

function ValueBadge({
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

function TopologyText({
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

function smartKubernetesName(name: string, kind: string): string {
	if (name.length <= 18) return name;
	const parts = name.split("-");
	const suffix = parts[parts.length - 1] ?? "";
	const generatedPodName =
		kind === "Pod" &&
		parts.length >= 3 &&
		/^[a-z0-9]{4,6}$/i.test(suffix);
	if (generatedPodName) {
		const prefix = parts.slice(0, -2).join("-");
		if (prefix.length >= 4) return `${prefix}...${suffix}`;
	}
	const generatedControllerName =
		["ReplicaSet", "Job"].includes(kind) &&
		parts.length >= 2 &&
		/^[a-z0-9]{6,12}$/i.test(suffix);
	if (generatedControllerName) {
		const prefix = parts.slice(0, -1).join("-");
		if (prefix.length >= 4) return `${prefix}-${suffix.slice(0, 3)}...`;
	}
	const headLength = name.length > 28 ? 15 : 14;
	const tailLength = name.length > 28 ? 5 : 4;
	return `${name.slice(0, headLength)}...${name.slice(-tailLength)}`;
}

function OwnershipResourceNode({
	data,
	selected,
}: NodeProps<OwnershipResourceGraphNode>) {
	const node = data.node;
	const visual = getResourceKindVisual(node.kind);
	const Icon = visual.icon;
	const selectedOrConnected = data.selected || data.connected;
	const metadataText = data.standalone
		? (node.namespace ?? "Cluster scoped")
		: node.namespace
			? `${node.kind} · ${node.namespace}`
			: node.kind;
	const displayName = smartKubernetesName(node.name, node.kind);
	const age = node.summary.age;
	const portHint = data.showPortHints
		? node.portHints?.slice(0, 3).join(", ")
		: undefined;

	return (
		<div
			className={cn(
				topologyNodeClassName(
					node,
					data.selected || selected ? node.id : null,
					node.id,
					selectedOrConnected,
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
			<TopologyText
				content={metadataText}
				className="text-[0.64rem] leading-tight text-muted-foreground"
			/>
			<div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
				{portHint || node.status ? (
					<ValueBadge
						value={portHint ?? node.status ?? ""}
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

function StandaloneKindGroupNode({
	data,
}: NodeProps<StandaloneKindGroupGraphNode>) {
	const visual = getResourceKindVisual(data.kind);
	const Icon = visual.icon;
	const ToggleIcon = data.expanded ? ChevronDown : ChevronRight;

	return (
		<div
			className={cn(
				"h-full w-full rounded-md border border-border/80 bg-card/35 shadow-sm transition-colors hover:bg-accent/20",
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

export const ownershipMapNodeTypes = {
	ownershipResource: OwnershipResourceNode,
	standaloneKindGroup: StandaloneKindGroupNode,
} satisfies NodeTypes;

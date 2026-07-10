import type {
	EdgeBase,
	NodeBase,
	SmoothStepPathOptions,
} from "@xyflow/system";
import type {
	ResourceSummary,
	TopologyNode,
	TopologyRelation,
} from "@/lib/types";
import type { TopologyGraph } from "./topology-graph";

export type TopologyStoplightTone = "success" | "warning" | "error" | "neutral";

export interface FlowTopologyNodeData extends Record<string, unknown> {
	node?: TopologyNode;
	resource: ResourceSummary | null;
	label: string;
	selected: boolean;
	connected: boolean;
	kind?: string;
	count?: number;
	nodeIds?: string[];
	expanded?: boolean;
	dimmed?: boolean;
	showPortHints?: boolean;
}

export type FlowTopologyNode = NodeBase<
	FlowTopologyNodeData,
	"ownershipResource" | "standaloneKindGroup"
> & {
	style?: string;
	focusable?: boolean;
};

export type FlowTopologyEdge = EdgeBase<
	{ relation: TopologyRelation },
	"smoothstep"
> & {
	pathOptions?: SmoothStepPathOptions;
	style?: string;
	focusable?: boolean;
};

export interface FlowTopology {
	nodes: FlowTopologyNode[];
	edges: FlowTopologyEdge[];
}

export interface FlowTopologySelectionIndex {
	graph: TopologyGraph;
}

export interface FlowTopologyBounds {
	left: number;
	top: number;
	right: number;
	bottom: number;
	width: number;
	height: number;
}

export interface BuildFlowTopologyOptions {
	expandedStandaloneKinds?: ReadonlySet<string>;
}

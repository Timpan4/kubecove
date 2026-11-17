import type {
	EdgeBase,
	EdgeMarker,
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
export type OwnershipMapViewportSize = { width: number; height: number };

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

export type FlowTopologyEdge = Omit<
	EdgeBase<{ relation: TopologyRelation }, "smoothstep">,
	"markerEnd"
> & {
	// Map edges always carry a marker definition, never a reference to an external marker id.
	markerEnd?: EdgeMarker;
	pathOptions?: SmoothStepPathOptions;
	style?: string;
	/** Set while a resource is selected: whether the edge is on its path. */
	class?: typeof ACTIVE_EDGE_CLASS | typeof INACTIVE_EDGE_CLASS;
	focusable?: boolean;
};

/** App.css moves the dashes of these edges on maps that carry `ownership-map-motion`. */
export const ACTIVE_EDGE_CLASS = "ownership-map-edge-active";
export const INACTIVE_EDGE_CLASS = "ownership-map-edge-inactive";

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

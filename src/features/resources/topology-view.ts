import type { CoordinateExtent } from "@xyflow/system";
import type { ResourceTopology, TopologyMode } from "@/lib/types";
import {
	applyFlowTopologySelectionWithIndex,
	buildFlowTopologyLayout,
	buildFlowTopologySelectionIndex,
	filterFlowTopologyToSelectedRoot,
	getTopologyTranslateExtent,
} from "./topology-implementation";
import type { FlowTopology, FlowTopologySelectionIndex } from "./topology-types";

type OwnershipMapViewportSize = { width: number; height: number };

export interface FlowTopologyViewOptions {
	mode: TopologyMode;
	selectedNodeId: string | null;
	showFullTopologyOnSelection: boolean;
	expandedStandaloneKinds: ReadonlySet<string>;
	viewportSize?: OwnershipMapViewportSize;
}

export interface FlowTopologyView {
	graph: FlowTopology;
	translateExtent?: CoordinateExtent;
}

/** Expensive topology work retained while ownership map stays mounted. */
export interface FlowTopologyLayoutState {
	layout: FlowTopology;
	selectionIndex: FlowTopologySelectionIndex;
}

function selectedStandaloneExpansionId(
	topology: ResourceTopology,
	selectedNodeId: string | null,
): string | null {
	if (!selectedNodeId || !topology.nodes.some((node) => node.id === selectedNodeId)) return null;
	return topology.edges.some(
		(edge) => edge.source === selectedNodeId || edge.target === selectedNodeId,
	)
		? null
		: selectedNodeId;
}

export function buildFlowTopologyView(
	topology: ResourceTopology,
	options: FlowTopologyViewOptions,
): FlowTopologyView {
	return buildFlowTopologyViewFromLayoutState(
		buildFlowTopologyLayoutState(
			topology,
			options.mode,
			options.expandedStandaloneKinds,
			selectedStandaloneExpansionId(topology, options.selectedNodeId),
		),
		options,
	);
}

export function buildFlowTopologyLayoutState(
	topology: ResourceTopology,
	mode: TopologyMode,
	expandedStandaloneKinds: ReadonlySet<string>,
	selectedNodeIdForExpansion: string | null,
): FlowTopologyLayoutState {
	return {
		layout: buildFlowTopologyLayout(topology, selectedNodeIdForExpansion, mode, {
			expandedStandaloneKinds,
		}),
		selectionIndex: buildFlowTopologySelectionIndex(topology),
	};
}

export function buildFlowTopologyViewFromLayoutState(
	state: FlowTopologyLayoutState,
	options: FlowTopologyViewOptions,
): FlowTopologyView {
	const visible = options.showFullTopologyOnSelection
		? state.layout
		: filterFlowTopologyToSelectedRoot(
				state.layout,
				state.selectionIndex,
				options.selectedNodeId,
			);
	const graph = applyFlowTopologySelectionWithIndex(
		visible,
		state.selectionIndex,
		options.selectedNodeId,
	);
	const viewportSize = options.viewportSize;
	return {
		graph,
		translateExtent:
			viewportSize && viewportSize.width > 0 && viewportSize.height > 0
				? getTopologyTranslateExtent(graph.nodes, viewportSize)
				: undefined,
	};
}

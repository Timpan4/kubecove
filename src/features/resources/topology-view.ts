import type { CoordinateExtent } from "@xyflow/system";
import type { ResourceTopology, TopologyMode } from "@/lib/types";
import {
	applyFlowTopologySelectionWithIndex,
	layoutFlowTopologyGraph,
	filterFlowTopologyToSelectedRoot,
	getTopologyTranslateExtent,
} from "./topology-implementation";
import { buildTopologyGraph, type TopologyGraph } from "./topology-graph";
import type {
	FlowTopology,
	FlowTopologySelectionIndex,
	OwnershipMapViewportSize,
} from "./topology-types";

export type { OwnershipMapViewportSize } from "./topology-types";

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

export function selectedStandaloneExpansionId(topology: ResourceTopology, selectedNodeId: string | null): string | null {
	return selectedStandaloneExpansionIdForGraph(buildTopologyGraph(topology), selectedNodeId);
}

export function selectedStandaloneExpansionIdForGraph(
	graph: TopologyGraph,
	selectedNodeId: string | null,
): string | null {
	if (!selectedNodeId || !graph.nodeIds.has(selectedNodeId)) return null;
	return graph.parents.has(selectedNodeId) || graph.children.has(selectedNodeId) ? null : selectedNodeId;
}

export function buildFlowTopologyView(
	topology: ResourceTopology,
	options: FlowTopologyViewOptions,
): FlowTopologyView {
	const graph = buildTopologyGraph(topology);
	return buildFlowTopologyViewFromLayoutState(
		buildFlowTopologyLayoutStateFromGraph(
			graph,
			options.mode,
			options.expandedStandaloneKinds,
			selectedStandaloneExpansionIdForGraph(graph, options.selectedNodeId),
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
	return buildFlowTopologyLayoutStateFromGraph(buildTopologyGraph(topology), mode, expandedStandaloneKinds, selectedNodeIdForExpansion);
}

export function buildFlowTopologyLayoutStateFromGraph(
	graph: TopologyGraph,
	mode: TopologyMode,
	expandedStandaloneKinds: ReadonlySet<string>,
	selectedNodeIdForExpansion: string | null,
): FlowTopologyLayoutState {
	return {
		layout: layoutFlowTopologyGraph(graph, selectedNodeIdForExpansion, mode, {
			expandedStandaloneKinds,
		}),
		selectionIndex: { graph },
	};
}

export function selectFlowTopology(
	state: FlowTopologyLayoutState,
	options: Pick<FlowTopologyViewOptions, "selectedNodeId" | "showFullTopologyOnSelection">,
): FlowTopology {
	const visible = options.showFullTopologyOnSelection
		? state.layout
		: filterFlowTopologyToSelectedRoot(
				state.layout,
				state.selectionIndex,
				options.selectedNodeId,
			);
	return applyFlowTopologySelectionWithIndex(
		visible,
		state.selectionIndex,
		options.selectedNodeId,
	);
}

export function buildFlowTopologyViewFromLayoutState(
	state: FlowTopologyLayoutState,
	options: FlowTopologyViewOptions,
): FlowTopologyView {
	const graph = selectFlowTopology(state, options);
	const viewportSize = options.viewportSize;
	return {
		graph,
		translateExtent:
			viewportSize && viewportSize.width > 0 && viewportSize.height > 0
				? getTopologyTranslateExtent(graph.nodes, viewportSize)
				: undefined,
	};
}

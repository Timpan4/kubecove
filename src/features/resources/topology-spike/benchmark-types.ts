export interface TopologySpikeRunResult {
	framework: "react" | "svelte";
	nodeCount: number;
	edgeCount: number;
	initialRenderMs: number;
	selectionChurnMs: number;
	viewportOpsMs: number;
	fitViewMs: number;
	totalInteractionMs: number;
	usedJsHeapBeforeBytes: number | null;
	usedJsHeapAfterBytes: number | null;
	renderedNodeCount: number;
	renderedEdgeCount: number;
}

export interface TopologySpikeWindow {
	__topologySpikeRun?: () => Promise<TopologySpikeRunResult>;
	__topologySpikeResult?: TopologySpikeRunResult;
}

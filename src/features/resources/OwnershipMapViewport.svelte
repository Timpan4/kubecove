<script lang="ts">
	import { tick } from "svelte";
	import { useSvelteFlow } from "@xyflow/svelte";
	import {
		getFlowTopologyBounds,
		topologyViewportFitKey,
		type FlowTopologyEdge,
		type FlowTopologyNode,
		widthFitFlowTopologyViewport,
	} from "./topologyModel";

	const FIT_VIEW_DURATION_MS = 260;
	const SELECTED_FIT_VIEW_DURATION_MS = 120;

	let {
		nodes,
		edges,
		selectedNodeId,
		viewportWidth,
		viewportHeight,
		viewportKey,
	}: {
		nodes: FlowTopologyNode[];
		edges: FlowTopologyEdge[];
		selectedNodeId: string | null;
		viewportWidth: number;
		viewportHeight: number;
		viewportKey: string;
	} = $props();

	const flow = useSvelteFlow<FlowTopologyNode, FlowTopologyEdge>();
	let lastFitKey = "";
	const selectedPathNodes = $derived(
		selectedNodeId
			? nodes.filter((node) => node.data.selected || node.data.connected)
			: [],
	);
	const nodesToFit = $derived(selectedPathNodes.length > 0 ? selectedPathNodes : nodes);
	const fitKey = $derived(
		topologyViewportFitKey(nodes, nodesToFit, edges, selectedNodeId, viewportKey),
	);
	const fitBounds = $derived(getFlowTopologyBounds(nodes, nodesToFit));
	const fitDuration = $derived(
		selectedPathNodes.length > 0 ? SELECTED_FIT_VIEW_DURATION_MS : FIT_VIEW_DURATION_MS,
	);

	$effect(() => {
		if (
			nodesToFit.length === 0 ||
			!fitBounds ||
			viewportWidth <= 0 ||
			viewportHeight <= 0 ||
			fitKey === lastFitKey ||
			typeof window === "undefined"
		) return;
		lastFitKey = fitKey;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			secondFrame = window.requestAnimationFrame(() => {
				void tick().then(() =>
					flow.setViewport(
						widthFitFlowTopologyViewport(fitBounds, {
							width: viewportWidth,
							height: viewportHeight,
						}),
						{ duration: fitDuration },
					),
				);
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	});
</script>

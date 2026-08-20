<script lang="ts">
	import { tick } from "svelte";
	import { prefersReducedMotion } from "svelte/motion";
	import { useSvelteFlow } from "@xyflow/svelte";
	import {
		buildFlowTopologyFitPlan,
		type FlowTopologyEdge,
		type FlowTopologyNode,
	} from "./topology";

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
	const fitPlan = $derived(
		buildFlowTopologyFitPlan(nodes, edges, selectedNodeId, viewportKey, {
			width: viewportWidth,
			height: viewportHeight,
		}),
	);
	const fitDuration = $derived(
		fitPlan?.focused ? SELECTED_FIT_VIEW_DURATION_MS : FIT_VIEW_DURATION_MS,
	);
	const viewportDuration = $derived(prefersReducedMotion.current ? 0 : fitDuration);

	$effect(() => {
		if (
			!fitPlan ||
			fitPlan.key === lastFitKey ||
			typeof window === "undefined"
		) return;
		lastFitKey = fitPlan.key;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			secondFrame = window.requestAnimationFrame(() => {
				void tick().then(() =>
					flow.setViewport(fitPlan.viewport, { duration: viewportDuration }),
				);
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	});
</script>

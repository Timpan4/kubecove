<script lang="ts">
	import { Controls, SvelteFlow, type Edge, type Node } from "@xyflow/svelte";
	import "@xyflow/svelte/dist/style.css";
	import type { TopologySpikeRunResult, TopologySpikeWindow } from "./benchmark-types";
	import SvelteBenchmarkController from "./SvelteBenchmarkController.svelte";
	import {
		createTopologySpikeGraph,
		type TopologySpikeGraph,
	} from "./synthetic-topology";
	import "./topology-spike.css";

	declare global {
		interface Window extends TopologySpikeWindow {}
	}

	const startedAt = performance.now();
	const nodeCount = Number(
		new URLSearchParams(window.location.search).get("nodes") ?? "1000",
	);
	const graph: TopologySpikeGraph = createTopologySpikeGraph(nodeCount);
	let selectedId: string | null = null;

	function selectNode(id: string | null) {
		selectedId = id;
	}

	function nodeStyle(selected: boolean, health: string): string {
		const border = selected
			? "2px solid #38bdf8"
			: "1px solid rgba(148, 163, 184, 0.55)";
		const background =
			health === "healthy"
				? "rgba(15, 23, 42, 0.94)"
				: "rgba(30, 41, 59, 0.96)";
		const shadow = selected ? "0 0 0 3px rgba(56, 189, 248, 0.22)" : "none";
		return [
			"width: 220px",
			"min-height: 74px",
			"padding: 8px 10px",
			"border-radius: 6px",
			`border: ${border}`,
			`background: ${background}`,
			`box-shadow: ${shadow}`,
			"color: #e5e7eb",
			"font-size: 11px",
			"line-height: 1.25",
			"white-space: pre-line",
		].join("; ");
	}

	$: nodes = graph.nodes.map<Node>((node) => {
		const selected = node.id === selectedId;
		return {
			id: node.id,
			type: "default",
			position: { x: node.x, y: node.y },
			data: {
				label: `${node.kind}\n${node.name}\n${node.namespace ?? "cluster"}`,
			},
			selected,
			draggable: false,
			connectable: false,
			style: nodeStyle(selected, node.health),
		};
	});

	$: edges = graph.edges.map<Edge>((edge) => {
		const active = edge.source === selectedId || edge.target === selectedId;
		const stroke = active ? "#38bdf8" : "rgba(148, 163, 184, 0.48)";
		const strokeWidth = active ? 2.4 : 1.4;
		return {
			id: edge.id,
			source: edge.source,
			target: edge.target,
			type: "smoothstep",
			animated: active,
			focusable: false,
			style: `stroke: ${stroke}; stroke-width: ${strokeWidth};`,
		};
	});

	export type { TopologySpikeRunResult };
</script>

<div class="topology-spike-shell">
	<div class="topology-spike-header">
		<strong>Svelte Flow topology spike</strong>
		<span>
			{graph.nodes.length} nodes / {graph.edges.length} edges / layout
			{Math.round(graph.layoutMs)} ms
		</span>
	</div>
	<div class="topology-spike-canvas">
		<SvelteFlow
			{nodes}
			{edges}
			fitView
			minZoom={0.1}
			maxZoom={1.5}
			nodesDraggable={false}
			nodesConnectable={false}
			elementsSelectable={true}
			onlyRenderVisibleElements={false}
			proOptions={{ hideAttribution: true }}
		>
			<Controls />
			<SvelteBenchmarkController {graph} onSelect={selectNode} {startedAt} />
		</SvelteFlow>
	</div>
	<pre id="result" class="topology-spike-result"></pre>
</div>

<script lang="ts">
	import { Controls, SvelteFlow, type Edge, type Node } from "@xyflow/svelte";
	import "@xyflow/svelte/dist/style.css";
	import type { TopologySpikeRunResult, TopologySpikeWindow } from "./benchmark-types";
	import BenchmarkController from "./BenchmarkController.svelte";
	import {
		createTopologySpikeGraph,
		type TopologySpikeGraph,
	} from "./synthetic-topology";
	import "./topology-spike.css";

	declare global {
		interface Window extends TopologySpikeWindow {}
	}

	const startedAt = performance.now();
	const searchParams = new URLSearchParams(window.location.search);
	const nodeCount = Number(
		searchParams.get("nodes") ?? "1000",
	);
	const edgeType = searchParams.get("edgeType") === "straight" ? "straight" : "smoothstep";
	const nodeDetail = searchParams.get("nodeDetail") === "compact" ? "compact" : "full";
	const edgeMode = searchParams.get("edgeMode") === "selected" ? "selected" : "all";
	const viewportMode = searchParams.get("viewport") === "focused" ? "focused" : "fit";
	const initialViewport = { x: 64, y: 72, zoom: 0.7 };
	const graph: TopologySpikeGraph = createTopologySpikeGraph(nodeCount);
	let selectedId: string | null = null;
	const shellClass =
		nodeDetail === "compact"
			? "topology-spike-shell topology-spike-shell--compact"
			: "topology-spike-shell";

	function selectNode(id: string | null) {
		selectedId = id;
	}

	function kindAbbreviation(kind: string): string {
		if (kind === "Ingress") return "ING";
		if (kind === "Service") return "SVC";
		if (kind === "Deployment") return "DEP";
		if (kind === "ReplicaSet") return "RS";
		if (kind === "Pod") return "POD";
		return kind.slice(0, 3).toUpperCase();
	}

	function nodeLabel(node: TopologySpikeGraph["nodes"][number]): string {
		if (nodeDetail === "compact") return kindAbbreviation(node.kind);
		return `${node.kind}\n${node.name}\n${node.namespace ?? "cluster"}`;
	}

	function nodeStyle(selected: boolean, health: string): string {
		const border = selected
			? "2px solid #bae6fd"
			: "1px solid rgba(148, 163, 184, 0.55)";
		const shadow = selected ? "0 0 0 3px rgba(56, 189, 248, 0.22)" : "none";
		if (nodeDetail === "compact") {
			const healthColor =
				health === "healthy"
					? "#14b8a6"
					: health === "attention"
						? "#f59e0b"
						: health === "restarted"
							? "#8b5cf6"
							: "#ef4444";
			const background = `linear-gradient(90deg, ${healthColor} 0 5px, rgba(15, 23, 42, 0.96) 5px 100%)`;
			return [
				"width: 48px",
				"height: 22px",
				"min-height: 22px",
				"padding: 0 6px 0 9px",
				"border-radius: 6px",
				`border: ${border}`,
				`background: ${background}`,
				`box-shadow: ${shadow}`,
				"color: #e5e7eb",
				"font-size: 10px",
				"font-weight: 700",
				"line-height: 20px",
				"text-align: center",
			].join("; ");
		}
		const background =
			health === "healthy"
				? "rgba(15, 23, 42, 0.94)"
				: "rgba(30, 41, 59, 0.96)";
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
				label: nodeLabel(node),
			},
			selected,
			draggable: false,
			connectable: false,
			style: nodeStyle(selected, node.health),
		};
	});

	$: visibleEdges =
		edgeMode === "selected"
			? graph.edges.filter(
					(edge) => selectedId !== null && (edge.source === selectedId || edge.target === selectedId),
				)
			: graph.edges;

	$: edges = visibleEdges.map<Edge>((edge) => {
		const active = edge.source === selectedId || edge.target === selectedId;
		const stroke = active ? "#38bdf8" : "rgba(148, 163, 184, 0.48)";
		const strokeWidth = active ? 2.4 : 1.4;
		return {
			id: edge.id,
			source: edge.source,
			target: edge.target,
			type: edgeType,
			animated: active,
			focusable: false,
			style: `stroke: ${stroke}; stroke-width: ${strokeWidth};`,
		};
	});

	export type { TopologySpikeRunResult };
</script>

<div class={shellClass}>
	<div class="topology-spike-header">
		<strong>Svelte Flow topology spike</strong>
		<span>
			{graph.nodes.length} nodes / {graph.edges.length} edges / layout
			{Math.round(graph.layoutMs)} ms / {viewportMode} viewport / {nodeDetail}
			nodes / {edgeMode} {edgeType} edges
		</span>
	</div>
	<div class="topology-spike-canvas">
		<SvelteFlow
			{nodes}
			{edges}
			fitView={viewportMode === "fit"}
			{initialViewport}
			minZoom={0.1}
			maxZoom={1.5}
			nodesDraggable={false}
			nodesConnectable={false}
			elementsSelectable={true}
			onlyRenderVisibleElements={true}
			proOptions={{ hideAttribution: true }}
		>
			<Controls />
			<BenchmarkController {graph} onSelect={selectNode} {startedAt} />
		</SvelteFlow>
	</div>
	<pre id="result" class="topology-spike-result"></pre>
</div>

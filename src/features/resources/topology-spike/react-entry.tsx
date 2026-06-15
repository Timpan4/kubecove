import { useCallback, useEffect, useMemo, useState } from "react";
import ReactDOM from "react-dom/client";
import {
	Controls,
	ReactFlow,
	useReactFlow,
	type Edge,
	type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import type {
	TopologySpikeRunResult,
	TopologySpikeWindow,
} from "./benchmark-types";
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
const graph = createTopologySpikeGraph(nodeCount);

function frame(): Promise<void> {
	return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

async function settle(): Promise<void> {
	await frame();
	await frame();
}

function jsHeapBytes(): number | null {
	const memory = (
		performance as Performance & { memory?: { usedJSHeapSize?: number } }
	).memory;
	return typeof memory?.usedJSHeapSize === "number"
		? memory.usedJSHeapSize
		: null;
}

function nodeStyle(selected: boolean, health: string): React.CSSProperties {
	return {
		width: 220,
		minHeight: 74,
		padding: "8px 10px",
		borderRadius: 6,
		border: selected ? "2px solid #38bdf8" : "1px solid rgba(148, 163, 184, 0.55)",
		background:
			health === "healthy"
				? "rgba(15, 23, 42, 0.94)"
				: "rgba(30, 41, 59, 0.96)",
		boxShadow: selected ? "0 0 0 3px rgba(56, 189, 248, 0.22)" : "none",
		color: "#e5e7eb",
		fontSize: 11,
		lineHeight: 1.25,
	};
}

function ReactTopologyController({
	graph,
	onSelect,
}: {
	graph: TopologySpikeGraph;
	onSelect: (id: string | null) => void;
}) {
	const flow = useReactFlow();

	useEffect(() => {
		const run = async (): Promise<TopologySpikeRunResult> => {
			await settle();
			const usedJsHeapBeforeBytes = jsHeapBytes();
			const initialRenderMs = performance.now() - startedAt;
			const selectionStarted = performance.now();

			for (const selectedId of graph.selectionIds) {
				onSelect(selectedId);
				await frame();
			}
			await settle();
			const selectionChurnMs = performance.now() - selectionStarted;

			const viewportStarted = performance.now();
			for (let index = 0; index < 24; index += 1) {
				await flow.setViewport(
					{
						x: -index * 26,
						y: -index * 14,
						zoom: 0.35 + (index % 5) * 0.08,
					},
					{ duration: 0 },
				);
			}
			await settle();
			const viewportOpsMs = performance.now() - viewportStarted;

			const fitStarted = performance.now();
			await flow.fitView({ padding: 0.2, duration: 0 });
			await settle();
			const fitViewMs = performance.now() - fitStarted;
			const result: TopologySpikeRunResult = {
				framework: "react",
				nodeCount: graph.nodes.length,
				edgeCount: graph.edges.length,
				initialRenderMs,
				selectionChurnMs,
				viewportOpsMs,
				fitViewMs,
				totalInteractionMs: selectionChurnMs + viewportOpsMs + fitViewMs,
				usedJsHeapBeforeBytes,
				usedJsHeapAfterBytes: jsHeapBytes(),
				renderedNodeCount: document.querySelectorAll(".react-flow__node").length,
				renderedEdgeCount: document.querySelectorAll(".react-flow__edge").length,
			};
			window.__topologySpikeResult = result;
			document.querySelector("#result")?.replaceChildren(
				document.createTextNode(JSON.stringify(result, null, 2)),
			);
			return result;
		};

		window.__topologySpikeRun = run;
		if (new URLSearchParams(window.location.search).has("autorun")) {
			void run();
		}
	}, [flow, graph, onSelect]);

	return null;
}

function ReactTopologySpike() {
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const nodes = useMemo<Node[]>(
		() =>
			graph.nodes.map((node) => {
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
			}),
		[selectedId],
	);
	const edges = useMemo<Edge[]>(
		() =>
			graph.edges.map((edge) => {
				const active = edge.source === selectedId || edge.target === selectedId;
				return {
					id: edge.id,
					source: edge.source,
					target: edge.target,
					type: "smoothstep",
					animated: active,
					focusable: false,
					style: {
						stroke: active ? "#38bdf8" : "rgba(148, 163, 184, 0.48)",
						strokeWidth: active ? 2.4 : 1.4,
					},
				};
			}),
		[selectedId],
	);
	const handleSelect = useCallback((id: string | null) => {
		setSelectedId(id);
	}, []);

	return (
		<div className="topology-spike-shell">
			<div className="topology-spike-header">
				<strong>React Flow topology spike</strong>
				<span>
					{graph.nodes.length} nodes / {graph.edges.length} edges / layout{" "}
					{Math.round(graph.layoutMs)} ms
				</span>
			</div>
			<div className="topology-spike-canvas">
				<ReactFlow
					nodes={nodes}
					edges={edges}
					fitView
					minZoom={0.1}
					maxZoom={1.5}
					nodesDraggable={false}
					nodesConnectable={false}
					elementsSelectable
					onlyRenderVisibleElements={false}
					proOptions={{ hideAttribution: true }}
				>
					<Controls />
					<ReactTopologyController graph={graph} onSelect={handleSelect} />
				</ReactFlow>
			</div>
			<pre id="result" className="topology-spike-result" />
		</div>
	);
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
	<ReactTopologySpike />,
);

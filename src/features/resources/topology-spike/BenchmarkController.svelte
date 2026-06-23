<script lang="ts">
	import { onMount, tick } from "svelte";
	import { useSvelteFlow } from "@xyflow/svelte";
	import type { TopologySpikeRunResult } from "./benchmark-types";
	import type { TopologySpikeGraph } from "./synthetic-topology";

	export let graph: TopologySpikeGraph;
	export let onSelect: (id: string | null) => void;
	export let startedAt: number;

	const flow = useSvelteFlow();

	function frame(): Promise<void> {
		return new Promise((resolve) => requestAnimationFrame(() => resolve()));
	}

	async function settle(): Promise<void> {
		await tick();
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

	onMount(() => {
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
				framework: "svelte",
				nodeCount: graph.nodes.length,
				edgeCount: graph.edges.length,
				initialRenderMs,
				selectionChurnMs,
				viewportOpsMs,
				fitViewMs,
				totalInteractionMs: selectionChurnMs + viewportOpsMs + fitViewMs,
				usedJsHeapBeforeBytes,
				usedJsHeapAfterBytes: jsHeapBytes(),
				renderedNodeCount: document.querySelectorAll(".svelte-flow__node").length,
				renderedEdgeCount: document.querySelectorAll(".svelte-flow__edge").length,
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
	});
</script>

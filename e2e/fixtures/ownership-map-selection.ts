import { flushSync, mount, unmount } from "svelte";
import type { ResourceSummary, ResourceTopology } from "@/lib/types";
import OwnershipMapHarness from "./OwnershipMapHarness.svelte";

// 15 Deployments with one ReplicaSet and two Pods each: 60 nodes and 45 edges.
// That stays under the map's visible-element culling threshold, so every node
// and edge stays mounted and a removed edge can only come from re-measurement.
const DEPLOYMENTS = 15;

export interface OwnershipMapRenderCounts {
	edges: number;
	edgeRemovals: number;
	handleMeasurements: number;
	updateMs: number;
}

let control: { stop(): Promise<void>; select(nodeId: string | null): void; refresh(): void; take(): OwnershipMapRenderCounts } | undefined;

function topology(): ResourceTopology {
	const nodes: ResourceTopology["nodes"] = [];
	const edges: ResourceTopology["edges"] = [];
	const add = (kind: string, name: string, owner?: string) => {
		const summary: ResourceSummary = {
			cluster: "ownership-map", namespace: "shop", kind, name, age: "5m", health: "healthy", status: "Running", ready: "1/1",
		};
		const id = `${kind}/${name}`;
		nodes.push({ id, kind, name, namespace: "shop", health: "healthy", status: "Running", selectable: true, summary });
		if (owner) edges.push({ id: `${owner}->${id}`, source: owner, target: id, relation: "owns" });
		return id;
	};
	for (let index = 0; index < DEPLOYMENTS; index++) {
		const deployment = add("Deployment", `api-${index}`);
		const replicaSet = add("ReplicaSet", `api-${index}-rs`, deployment);
		add("Pod", `api-${index}-rs-a`, replicaSet);
		add("Pod", `api-${index}-rs-b`, replicaSet);
	}
	return { nodes, edges, warnings: [] };
}

export async function mountOwnershipMap(): Promise<void> {
	await control?.stop();
	const target = document.createElement("div");
	document.body.append(target);
	const component = mount(OwnershipMapHarness, { target, props: { initialTopology: topology() } });
	const counts = { edgeRemovals: 0, handleMeasurements: 0, updateMs: 0 };
	// Time the synchronous Svelte update that applies one change to the map.
	const timed = (change: () => void) => {
		const start = performance.now();
		flushSync(change);
		counts.updateMs += performance.now() - start;
	};
	// Svelte Flow measures handle bounds with getBoundingClientRect on each handle.
	const measure = Element.prototype.getBoundingClientRect;
	Element.prototype.getBoundingClientRect = function (this: Element) {
		if (this.classList.contains("svelte-flow__handle")) counts.handleMeasurements++;
		return measure.call(this);
	};
	const observer = new MutationObserver((records) => {
		for (const record of records) {
			for (const node of record.removedNodes) {
				if (node instanceof Element && node.matches(".svelte-flow__edge")) counts.edgeRemovals++;
			}
		}
	});
	observer.observe(target, { childList: true, subtree: true });
	control = {
		select: (nodeId) => timed(() => component.select(nodeId)),
		refresh: () => timed(() => component.refresh(topology())),
		take() {
			const taken = { edges: target.querySelectorAll(".svelte-flow__edge").length, ...counts };
			counts.edgeRemovals = 0;
			counts.handleMeasurements = 0;
			counts.updateMs = 0;
			return taken;
		},
		async stop() {
			observer.disconnect();
			Element.prototype.getBoundingClientRect = measure;
			await unmount(component);
			target.remove();
			control = undefined;
		},
	};
}

/** Wait for Svelte Flow to settle, then return and reset the counts since the last call. */
export async function settleOwnershipMap(): Promise<OwnershipMapRenderCounts> {
	if (!control) throw new Error("Ownership map fixture is not mounted");
	// FIT_VIEW_DURATION_MS in OwnershipMapViewport.svelte is the longest viewport
	// animation; the frames after it cover ResizeObserver and re-render work.
	await new Promise((resolve) => setTimeout(resolve, 260));
	for (let frame = 0; frame < 3; frame++) await new Promise((resolve) => requestAnimationFrame(resolve));
	return control.take();
}

export function selectOwnershipMapNode(nodeId: string | null): void { control?.select(nodeId); }
export function refreshOwnershipMapTopology(): void { control?.refresh(); }
export async function stopOwnershipMap(): Promise<void> { await control?.stop(); }

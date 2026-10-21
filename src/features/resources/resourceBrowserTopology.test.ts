import { expect, test } from "bun:test";
import { createTableTopologyFilter } from "./resourceBrowserModel";
import type { ResourceSummary, ResourceTopology } from "@/lib/types";

// Failure modes: sorting relayouts the same graph; new topology or membership is ignored;
// ancestors disappear when only a child matches; hiding historical replicas is ignored.
test("topology membership memo preserves ancestors and responds to graph and filter changes", () => {
	const parent: ResourceSummary = {
		cluster: "dev",
		namespace: null,
		kind: "Deployment",
		name: "parent",
		age: "1m",
		health: "healthy",
	};
	const child = { ...parent, kind: "Pod", name: "child" };
	const topology: ResourceTopology = {
		warnings: [],
		nodes: [parent, child].map((summary) => ({
			...summary,
			id: summary.name,
			summary,
			selectable: true,
		})),
		edges: [
			{ id: "owns", source: "parent", target: "child", relation: "owns" },
		],
	};
	const filter = createTableTopologyFilter();
	const first = filter(topology, [child], false);
	expect(first?.nodes.map((node) => node.id)).toEqual(["parent", "child"]);
	expect(filter(topology, [{ ...child }], false)).toBe(first);
	const both = filter(topology, [child, parent], false);
	expect(filter(topology, [parent, child], false)).toBe(both);
	expect(
		filter(topology, [parent], false)?.nodes.map((node) => node.id),
	).toEqual(["parent"]);
	const changed = {
		...topology,
		nodes: topology.nodes.map((node) => ({
			...node,
			name: `${node.name}-updated`,
		})),
	};
	expect(filter(changed, [child], false)).not.toBe(first);
	expect(filter(undefined, [child], false)).toBeUndefined();
});

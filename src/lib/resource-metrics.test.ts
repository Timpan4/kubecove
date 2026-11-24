import { expect, test } from "bun:test";
import {
	resourceMetricFor,
	resourceMetricIndex,
} from "./resource-metrics";
import { buildResourceSearchIndex } from "@/features/resources/helpers";
import type {
	ResourceMetricSummary,
	ResourceSummary,
} from "./types";

test("metrics lookups publish changed samples and fall back when samples expire", () => {
	const row: ResourceSummary = {
		cluster: "dev",
		namespace: "ns",
		kind: "Pod",
		name: "pod",
		age: "1m",
		health: "healthy",
	};
	const metric: ResourceMetricSummary = {
		cluster: "dev",
		namespace: "ns",
		kind: "Pod",
		name: "pod",
		cpuMillicores: 1,
		sourcePods: ["pod"],
	};
	const index = (value: ResourceMetricSummary) =>
		resourceMetricIndex([{ clusterContext: "dev", summary: {
			cluster: "dev",
			availability: { status: "available" },
			pods: [value],
			nodes: [],
			workloads: [],
			warnings: [],
		} }]);
	for (const changed of [
		metric,
		{ ...metric, cpuMillicores: 2 },
		{ ...metric, sampledAt: "now" },
		{ ...metric, sourcePods: ["another"] },
	]) {
		expect(resourceMetricFor(index(changed), row)).toBe(changed);
	}
	expect(resourceMetricFor(new Map(), row)).toBeUndefined();
	expect(resourceMetricFor(new Map(), row, metric)).toBe(metric);
	expect(row.metrics).toBeUndefined();
});

test("search entries retain identity for unchanged rows and replace changed search terms", () => {
	const row: ResourceSummary = {
		cluster: "dev",
		namespace: null,
		kind: "Pod",
		name: "old-name",
		age: "1m",
		health: "healthy",
	};
	const first = buildResourceSearchIndex([row])[0];
	expect(buildResourceSearchIndex([row])[0]).toBe(first);
	const changed = buildResourceSearchIndex([{ ...row, name: "new-name" }])[0];
	expect(changed.searchText).toContain("new-name");
	expect(changed.searchText).not.toContain("old-name");
});

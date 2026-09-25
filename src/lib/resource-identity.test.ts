import { describe, expect, test } from "bun:test";
import { dynamicKindKey, findResourceIndex, looseResourceKey, resourceKey } from "./resource-identity";
import type { DiscoveredResourceKind, ResourceSummary } from "./types";

const resource = (overrides: Partial<ResourceSummary> = {}): ResourceSummary => ({
	cluster: "kind-dev",
	kind: "Application",
	name: "api",
	namespace: "argocd",
	age: "1d",
	health: "unknown",
	...overrides,
});
const self = (summary: ResourceSummary) => summary;

describe("resource identity", () => {
	test("an exact match wins over an earlier match without apiVersion", () => {
		const target = resource({ apiVersion: "argoproj.io/v1alpha1" });
		const candidates = [resource({ apiVersion: "app.k8s.io/v1beta1" }), target];

		expect(findResourceIndex(candidates, target, self)).toBe(1);
	});

	test("falls back to a candidate whose source omitted apiVersion", () => {
		const target = resource({ kind: "DaemonSet", apiVersion: "apps/v1" });
		const candidates = [resource({ kind: "Pod" }), resource({ kind: "DaemonSet" })];

		expect(findResourceIndex(candidates, target, self)).toBe(1);
		expect(looseResourceKey(candidates[1])).toBe(looseResourceKey(target));
		expect(resourceKey(candidates[1])).not.toBe(resourceKey(target));
	});

	test("does not match another cluster, namespace, or name", () => {
		const target = resource();
		const candidates = [
			resource({ cluster: "kind-prod" }),
			resource({ namespace: "default" }),
			resource({ namespace: undefined }),
			resource({ name: "api-2" }),
		];

		expect(findResourceIndex(candidates, target, self)).toBe(-1);
	});

	test("matches through a projection", () => {
		const target = resource();
		const nodes = [{ id: "other", summary: resource({ name: "web" }) }, { id: "node", summary: target }];

		expect(findResourceIndex(nodes, target, (node) => node.summary)).toBe(1);
	});

	test("dynamic kind keys distinguish plural and scope", () => {
		const widget: DiscoveredResourceKind = {
			group: "example.com",
			version: "v1",
			apiVersion: "example.com/v1",
			kind: "Widget",
			plural: "widgets",
			shortNames: [],
			namespaced: true,
		};

		expect(dynamicKindKey(null)).toBe("");
		expect(dynamicKindKey(widget)).not.toBe(dynamicKindKey({ ...widget, plural: "widgetz" }));
		expect(dynamicKindKey(widget)).not.toBe(dynamicKindKey({ ...widget, namespaced: false }));
	});
});

import { describe, expect, test } from "bun:test";
import {
	isClusterScopedKind,
	kindFetchKeys,
	kindKey,
	kindLabel,
	kindMatcher,
	uniqueKinds,
} from "./kind-identity";
import { sanitizePathStateTreeNode } from "./path-state";
import type { DiscoveredResourceKind } from "./types";

const widget: DiscoveredResourceKind = {
	group: "example.com",
	version: "v1",
	apiVersion: "example.com/v1",
	kind: "Widget",
	plural: "widgets",
	shortNames: [],
	namespaced: true,
};

describe("kind identity", () => {
	test("encodes one kind to one key however it was listed", () => {
		const relisted = { ...widget, shortNames: ["wd"] };

		expect(kindKey(relisted)).toBe(kindKey(widget));
		expect(uniqueKinds(["Pod", widget, relisted])).toEqual(["Pod", widget]);
	});

	test("keeps a built-in kind and a Custom Resources kind with the same name apart", () => {
		const podCrd: DiscoveredResourceKind = {
			...widget,
			kind: "Pod",
			plural: "pods",
		};

		expect(kindKey(podCrd)).not.toBe(kindKey("Pod"));
		expect(uniqueKinds(["Pod", podCrd])).toHaveLength(2);
	});

	test("matches a resource by apiVersion, or by kind name when the resource has no apiVersion", () => {
		const matches = kindMatcher([widget]);

		expect(matches({ kind: "Widget", apiVersion: "example.com/v1" })).toBe(true);
		expect(matches({ kind: "Widget", apiVersion: "other.example.com/v1" })).toBe(false);
		expect(matches({ kind: "Widget", apiVersion: undefined })).toBe(true);
		expect(matches({ kind: "Gadget", apiVersion: "example.com/v1" })).toBe(false);
	});

	test("matches a built-in kind by its name", () => {
		const matches = kindMatcher(["Pod"]);

		expect(matches({ kind: "Pod", apiVersion: "v1" })).toBe(true);
		expect(matches({ kind: "Service", apiVersion: "v1" })).toBe(false);
	});

	test("labels a built-in kind by its name and a Custom Resources kind by its kind", () => {
		expect(kindLabel("Pod")).toBe("Pod");
		expect(kindLabel(widget)).toBe("Widget");
	});

	test("reads a cluster-scoped kind once and a namespaced kind once per namespace", () => {
		expect(kindFetchKeys(["a", "b"], ["Node", "Pod"])).toEqual([
			{ kind: "Node" },
			{ kind: "Pod", namespace: "a" },
			{ kind: "Pod", namespace: "b" },
		]);
		expect(isClusterScopedKind({ ...widget, namespaced: false })).toBe(true);
		expect(isClusterScopedKind("Pod")).toBe(false);
	});

	test("reads a tree node saved with the older custom kind key as the current key", () => {
		// Earlier builds saved group/version/plural/kind, which is the same text as apiVersion/plural/kind.
		const saved = {
			type: "kind",
			section: "discovered",
			kind: "example.com/v1/widgets/Widget",
			resourceKind: widget,
		};

		const node = sanitizePathStateTreeNode(saved);

		expect(node?.kind).toBe(kindKey(widget));
		expect(node?.resourceKind).toEqual(widget);
	});

	test("reads a tree node saved with a built-in kind name unchanged", () => {
		const node = sanitizePathStateTreeNode({ type: "kind", section: "workloads", kind: "Pod" });

		expect(node?.kind).toBe("Pod");
	});
});

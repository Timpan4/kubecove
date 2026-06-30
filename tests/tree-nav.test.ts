import { describe, expect, test } from "bun:test";
import {
	emptyStateMessage,
	resolveTreeScope,
	type TreeNodeId,
} from "../src/lib/tree-nav";
import type { DiscoveredResourceKind } from "../src/lib/types";

describe("tree navigation scope helpers", () => {
	test("resolves null scope to no query", () => {
		expect(resolveTreeScope(null)).toEqual({
			section: null,
			namespace: null,
			group: null,
			kinds: [],
			clusterScoped: false,
			argoMode: false,
			helmMode: false,
			incidentMode: false,
			portForwardMode: false,
			rbacMode: false,
		});
	});

	test("resolves namespace nodes to all namespaced supported kinds", () => {
		const scope = resolveTreeScope({
			type: "namespace",
			section: "namespaces",
			namespace: "payments",
		});
		expect(scope.namespace).toBe("payments");
		expect(scope.clusterScoped).toBe(false);
		expect(scope.argoMode).toBe(false);
		expect(scope.kinds).toContain("Pod");
		expect(scope.kinds).not.toContain("Node");
	});

	test("resolves group and kind nodes", () => {
		expect(
			resolveTreeScope({
				type: "group",
				section: "namespaces",
				namespace: "default",
				group: "Workloads",
			}),
		).toMatchObject({
			section: "namespaces",
			namespace: "default",
			group: "Workloads",
			clusterScoped: false,
		});

		expect(
			resolveTreeScope({
				type: "kind",
				section: "namespaces",
				namespace: "default",
				group: "Workloads",
				kind: "Deployment",
			}),
		).toMatchObject({
			section: "namespaces",
			namespace: "default",
			group: "Workloads",
			kinds: ["Deployment"],
			clusterScoped: false,
		});
	});

	test("resolves cluster-scoped and Argo nodes", () => {
		expect(
			resolveTreeScope({
				type: "kind",
				section: "clusterOverview",
				kind: "Node",
			}),
		).toMatchObject({
			section: "clusterOverview",
			namespace: null,
			kinds: ["Node"],
			clusterScoped: true,
			argoMode: false,
		});

		expect(resolveTreeScope({ type: "section", section: "argo" })).toMatchObject({
			section: "argo",
			argoMode: true,
		});
		expect(resolveTreeScope({ type: "section", section: "helm" })).toMatchObject({
			section: "helm",
			helmMode: true,
		});
		expect(
			resolveTreeScope({ type: "section", section: "incidents" }),
		).toMatchObject({
			section: "incidents",
			incidentMode: true,
		});
		expect(
			resolveTreeScope({ type: "section", section: "portForwards" }),
		).toMatchObject({
			section: "portForwards",
			portForwardMode: true,
		});
		expect(resolveTreeScope({ type: "section", section: "rbac" })).toMatchObject({
			section: "rbac",
			rbacMode: true,
		});
	});

	test("resolves discovered resource kind nodes", () => {
		const resourceKind: DiscoveredResourceKind = {
			group: "example.com",
			version: "v1",
			apiVersion: "example.com/v1",
			kind: "Widget",
			plural: "widgets",
			namespaced: true,
		};

		expect(
			resolveTreeScope({
				type: "kind",
				section: "discovered",
				kind: "example.com/v1/widgets/Widget",
				resourceKind,
			}),
		).toMatchObject({
			section: "discovered",
			namespace: null,
			kinds: [resourceKind],
			clusterScoped: false,
			argoMode: false,
		});
	});

	test("explains empty states from scope", () => {
		expect(emptyStateMessage(resolveTreeScope(null), false)).toBe(
			"Select a cluster context first",
		);
		expect(
			emptyStateMessage(
				resolveTreeScope({
					type: "section",
					section: "namespaces",
				} as TreeNodeId),
				true,
			),
		).toBe("Loading all namespaces");
		expect(
			emptyStateMessage(
				resolveTreeScope({ type: "section", section: "argo" } as TreeNodeId),
				true,
			),
		).toBe("Select a GitOps resource type");
		expect(
			emptyStateMessage(
				resolveTreeScope({ type: "section", section: "helm" } as TreeNodeId),
				true,
			),
		).toBe("Select a Helm resource type");
		expect(
			emptyStateMessage(
				resolveTreeScope({
					type: "section",
					section: "incidents",
				} as TreeNodeId),
				true,
			),
		).toBe("Use the Incident Cockpit");
		expect(
			emptyStateMessage(
				resolveTreeScope({ type: "section", section: "rbac" } as TreeNodeId),
				true,
			),
		).toBe("Select an RBAC inspection view");
		expect(
			emptyStateMessage(
				resolveTreeScope({
					type: "section",
					section: "discovered",
				} as TreeNodeId),
				true,
			),
		).toBe("Select a custom resource kind");
	});
});

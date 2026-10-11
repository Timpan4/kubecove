import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	createWorkspaceNavigation,
	navigateWorkspace,
	workspaceNavigationSnapshot,
} from "@/app/svelte/workspaceNavigation";
import type { ResourceTableState } from "@/features/resources/resourceTableModel";
import {
	buildResourceTableModel,
	buildResourceTableProjection,
} from "@/features/resources/resourceTableModel";
import type {
	PathStateResourceBrowserState,
	PathStateResourceDetailState,
	PathStateSnapshot,
	PathStateSurfacesState,
} from "@/lib/path-state";
import {
	decodePathStateSnapshot,
	defaultPathStateSnapshot,
	encodePathStateSnapshot,
	sanitizePathStateSnapshot,
} from "@/lib/path-state";
import { nodeIdToString } from "@/lib/tree-nav";
import { savedWorkspace, workspaceRows } from "./workspace-benchmark-fixtures";

const workspace = savedWorkspace(0, 100);
const tableProjection = buildResourceTableProjection(workspaceRows);
const tableState: ResourceTableState = {
	search: "resource",
	gitOpsFilter: "",
	healthFilter: "all",
	sort: { id: "name", desc: false },
	pageIndex: 4,
	collapsedGroups: new Set(["namespace-1:Deployment"]),
	selectedResource: workspaceRows[42],
};
const resourcePathState: PathStateResourceBrowserState = {
	selectedNamespaces: Array.from(
		{ length: 100 },
		(_, index) => `namespace-${index}`,
	),
	selectedKinds: [
		"Pod",
		"Deployment",
		"ReplicaSet",
		"Service",
		"ConfigMap",
		"Secret",
		...Array.from({ length: 100 }, (_, index) => ({
			group: `operator-${index % 40}.example.com`,
			version: index % 2 === 0 ? "v1" : "v1beta1",
			apiVersion: `operator-${index % 40}.example.com/${index % 2 === 0 ? "v1" : "v1beta1"}`,
			kind: `Widget${index}`,
			plural: `widgets${index}`,
			namespaced: index % 5 !== 0,
		})),
	],
	search: "checkout",
	gitOpsFilter: "argo:Application:argocd:checkout",
	healthFilter: "degraded",
	sortColumn: "memory",
	sortDesc: true,
	pageIndex: 4,
	scopeEditorOpen: true,
	collapsedGroups: ["namespace-1:Deployment", "namespace-9:Pod"],
	topologyMode: "ownership",
	selectedTopologyNodeId: "prod/v1/Pod/namespace-42/resource-42",
	mapPanelOpen: true,
	tablePanelOpen: true,
};
const detail: PathStateResourceDetailState = {
	activeTab: "logs",
	metadataLabelsExpanded: true,
	metadataAnnotationsExpanded: false,
	selectedContainer: "api",
	logFilter: "timeout",
	logWrapLines: true,
	logLatestFirst: true,
	yamlViewMode: "applyClean",
	yamlEncoding: "yaml",
	yamlShowFullDiff: true,
};
const surfaces: PathStateSurfacesState = {
	incidentFilter: "warning",
	helmSearch: "checkout",
	selectedHelmRelease: { name: "checkout-api", namespace: "checkout" },
	selectedGitOpsApplication: "checkout",
	rbac: {
		riskBucket: "high",
		selectedObjectKey: "ServiceAccount:checkout:api",
	},
};
const expandedSections = Array.from({ length: 100 }, (_, index) =>
	nodeIdToString({
		type: "namespace",
		section: "namespaces",
		namespace: `namespace-${index}`,
	}),
);
const navigation = {
	...createWorkspaceNavigation(workspace),
	selectedNode: {
		type: "kind" as const,
		section: "workloads" as const,
		kind: "Deployment",
	},
	viewMode: "resources" as const,
	resourceInitialSearch: "checkout",
	resourceInitialGitOpsFilter: "argo:Application:argocd:checkout",
	resourceInitialHealthFilter: "degraded" as const,
	resourceNamespaceOverride: resourcePathState.selectedNamespaces,
	resourceBrowserPathState: resourcePathState,
};
const workspaceSnapshot = workspaceNavigationSnapshot(navigation, {
	workspaceId: workspace.id,
	expandedSections,
	detail,
	surfaces,
});
const snapshot: PathStateSnapshot = {
	...defaultPathStateSnapshot(),
	workspace: workspaceSnapshot,
};
const sanitized = sanitizePathStateSnapshot(snapshot);
const encoded = encodePathStateSnapshot(snapshot);
const malformedEncodedSnapshot = "{";
const wrongVersionSnapshot = JSON.stringify({
	...snapshot,
	version: snapshot.version + 1,
});
const decoded = decodePathStateSnapshot(encoded);
const samePathNavigation = navigateWorkspace(navigation, {
	type: "updateResourceBrowserPath",
	pathState: resourcePathState,
});
const changedPathNavigation = navigateWorkspace(navigation, {
	type: "updateResourceBrowserPath",
	pathState: {
		...resourcePathState,
		pageIndex: resourcePathState.pageIndex + 1,
	},
});

assert.equal(workspaceRows.length, 10_000);
assert.equal(
	buildResourceTableModel(tableProjection, tableState).filteredRows.length,
	10_000,
);
assert(sanitized && decoded);
assert.equal(decodePathStateSnapshot(malformedEncodedSnapshot), null);
assert.equal(decodePathStateSnapshot(wrongVersionSnapshot), null);
assert.equal(encoded.length, JSON.stringify(sanitized).length);
assert.equal(samePathNavigation, navigation);
assert.notEqual(changedPathNavigation, navigation);
assert.equal(decoded?.workspace?.resources?.selectedKinds.length, 106);
assert.equal(decoded?.workspace?.expandedSections.length, 100);

describe("F02 retained table state with folded F29 workspace path snapshot", () => {
	bench(
		"F02 table interaction plus F29 snapshot comparison and serialization",
		() => {
			buildResourceTableModel(tableProjection, tableState);
			const current = workspaceNavigationSnapshot(navigation, {
				workspaceId: workspace.id,
				expandedSections,
				detail,
				surfaces,
			});
			navigateWorkspace(navigation, {
				type: "updateResourceBrowserPath",
				pathState: resourcePathState,
			});
			const nextSnapshot: PathStateSnapshot = {
				...snapshot,
				workspace: current,
			};
			const clean = sanitizePathStateSnapshot(nextSnapshot);
			if (clean) decodePathStateSnapshot(encodePathStateSnapshot(clean));
		},
	);

	bench("F29 changed path-state comparison and persisted encode/decode", () => {
		const changed = navigateWorkspace(navigation, {
			type: "updateResourceBrowserPath",
			pathState: {
				...resourcePathState,
				pageIndex: resourcePathState.pageIndex + 1,
			},
		});
		const nextWorkspace = workspaceNavigationSnapshot(changed, {
			workspaceId: workspace.id,
			expandedSections,
			detail,
			surfaces,
		});
		const next: PathStateSnapshot = { ...snapshot, workspace: nextWorkspace };
		decodePathStateSnapshot(encodePathStateSnapshot(next));
		decodePathStateSnapshot(malformedEncodedSnapshot);
		decodePathStateSnapshot(wrongVersionSnapshot);
	});
});

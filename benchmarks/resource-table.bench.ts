import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { argoResourceIdentityKey } from "@/features/gitops/argo-workspace-model";
import {
	buildResourceSearchIndex,
	filterResourceSearchIndex,
	resourceGroupCollapseKey,
} from "@/features/resources/helpers";
import { filterResourcesByKinds } from "@/features/resources/resourceBrowserModel";
import {
	buildResourceTableModel,
	buildResourceTableProjection,
	type ResourceTableState,
} from "@/features/resources/resourceTableModel";
import { gitOpsOwnershipGroupLabel } from "@/lib/gitops-ownership-evidence";
import type {
	DiscoveredResourceKind,
	HealthAssessment,
	ResourceKindSelection,
	ResourceMetricSummary,
	ResourceSummary,
} from "@/lib/types";

const kinds = [
	"Deployment",
	"Pod",
	"Service",
	"ConfigMap",
	"Ingress",
	"Widget",
];

const widgetKind: DiscoveredResourceKind = {
	group: "operator.example.com",
	version: "v1beta1",
	apiVersion: "operator.example.com/v1beta1",
	kind: "Widget",
	plural: "widgets",
	namespaced: true,
};

function healthAssessment(state: "degraded" | "healthy"): HealthAssessment {
	return {
		state,
		completeness: "complete",
		winningSources: ["kubernetes"],
		reasons: [state === "healthy" ? "Ready=True" : "Ready=False"],
		evidence: [
			{
				source: "kubernetes",
				raw: { type: "Ready", status: state === "healthy" ? "True" : "False" },
				state,
				current: true,
				reason: state === "healthy" ? "Ready=True" : "Ready=False",
			},
		],
	};
}

function metricSummary(
	kind: string,
	name: string,
	namespace: string,
	index: number,
): ResourceMetricSummary {
	return {
		kind,
		cluster: "prod",
		name,
		namespace,
		cpuMillicores: (index % 64) * 10,
		memoryBytes: (index % 512) * 1024 * 1024,
		sourcePods: [],
	};
}

function resourceSummary(index: number): ResourceSummary {
	const kind = kinds[index % kinds.length] ?? "Pod";
	const namespace = `namespace-${index % 100}`;
	const app = `checkout-${index % 250}`;
	const healthState = index % 17 === 0 ? "degraded" : "healthy";
	const name = `${app}-${kind.toLowerCase()}-${index}`;
	return {
		cluster: "prod",
		apiVersion:
			kind === "Pod" || kind === "Service" || kind === "ConfigMap"
				? "v1"
				: kind === "Widget"
					? widgetKind.apiVersion
					: "apps/v1",
		kind,
		name,
		namespace,
		age: `${index % 60}m`,
		health: healthState,
		createdAt: `2026-01-${String((index % 28) + 1).padStart(2, "0")}T00:00:00Z`,
		status: index % 17 === 0 ? "CrashLoopBackOff" : "Running",
		healthAssessment: healthAssessment(healthState),
		ready: index % 17 === 0 ? "False" : "True",
		restarts: index % 17 === 0 ? 5 : index % 3,
		ownerRef: kind === "Pod" ? `${app}-deployment` : undefined,
		argoApp: index % 3 === 0 ? app : undefined,
		gitOpsOwner:
			index % 3 === 0
				? {
						provider: "argo",
						kind: "Application",
						name: app,
						namespace: "argocd",
						confidence: "metadata",
					}
				: undefined,
		metrics: metricSummary(kind, name, namespace, index),
	};
}

const rows = Array.from({ length: 10_000 }, (_, index) =>
	resourceSummary(index),
);
const widgetRow = rows.find((row) => row.kind === "Widget");
const collapsedRow = rows[1];
if (!widgetRow || !collapsedRow) throw new Error("Missing table fixture rows");
const kindRows = [
	...rows,
	{
		...widgetRow,
		name: "wrong-widget-version",
		apiVersion: "operator.example.com/v1",
	},
];
const lateKindRows = [...kindRows.filter((row) => row !== widgetRow), widgetRow];
const unsupportedWidgetKind: DiscoveredResourceKind = {
	...widgetKind,
	version: "v2",
	apiVersion: "operator.example.com/v2",
};
const selectedKinds: ResourceKindSelection[] = ["Pod", widgetKind];
const searchIndex = buildResourceSearchIndex(rows);
const projection = buildResourceTableProjection(rows);
const gitOpsFilter = "argo:Application:argocd:checkout-42";
const state: ResourceTableState = {
	search: "checkout",
	gitOpsFilter: "",
	healthFilter: "degraded",
	sort: { id: "memory", desc: true },
	pageIndex: 0,
	collapsedGroups: new Set(),
	selectedResource: rows[42],
};

const gitOpsState: ResourceTableState = {
	...state,
	search: "checkout",
	gitOpsFilter,
	healthFilter: "all",
};

const selectedAndCollapsedState: ResourceTableState = {
	...state,
	search: "",
	healthFilter: "all",
	sort: { id: "cpu", desc: true },
	pageIndex: 3,
	collapsedGroups: new Set([resourceGroupCollapseKey(collapsedRow)]),
	selectedResource: rows[42],
	preferredGitOpsResourceKeys: new Set([
		"prod/apps/v1/Deployment/namespace-42/resource-42",
	]),
};

const stateVariants: ResourceTableState[] = [
	state,
	gitOpsState,
	selectedAndCollapsedState,
	{ ...state, search: "checkout-42", healthFilter: "all", pageIndex: 2 },
	{
		...state,
		search: "",
		healthFilter: "healthy",
		sort: { id: "name", desc: false },
	},
];

const ownershipRows: ResourceSummary[] = Array.from(
	{ length: 2_500 },
	(_, index) => {
		const namespace = `namespace-${index % 100}`;
		const app = `checkout-${index}`;
		const unhealthy = index % 5 === 0;
		const health: ResourceSummary["health"] = unhealthy
			? "degraded"
			: "healthy";
		const shared = {
			cluster: "prod",
			namespace,
			age: "1m",
			status: unhealthy ? "CrashLoopBackOff" : "Running",
			health,
			healthAssessment: healthAssessment(unhealthy ? "degraded" : "healthy"),
			ready: unhealthy ? "False" : "True",
		};
		const deploymentName = `${app}-deployment`;
		const replicaSetName = `${app}-replicaset`;
		return [
			{
				...shared,
				apiVersion: "apps/v1",
				kind: "Deployment",
				name: deploymentName,
				metrics: metricSummary("Deployment", deploymentName, namespace, index),
				argoApp: app,
				gitOpsOwner: {
					provider: "argo" as const,
					kind: "Application",
					name: app,
					namespace: "argocd",
					confidence: "metadata",
				},
			},
			{
				...shared,
				apiVersion: "apps/v1",
				kind: "ReplicaSet",
				name: replicaSetName,
				metrics: metricSummary("ReplicaSet", replicaSetName, namespace, index),
				ownerRef: deploymentName,
			},
			{
				...shared,
				apiVersion: "v1",
				kind: "Pod",
				name: `${app}-pod`,
				metrics: metricSummary("Pod", `${app}-pod`, namespace, index),
				ownerRef: replicaSetName,
				restarts: unhealthy ? 5 : 0,
			},
			{
				...shared,
				apiVersion: "v1",
				kind: "Service",
				name: `${app}-service`,
				metrics: metricSummary("Service", `${app}-service`, namespace, index),
				ownerRef: deploymentName,
			},
		] satisfies ResourceSummary[];
	},
).flat();
function ownershipEdgeResource(
	kind: string,
	name: string,
	ownerRef?: string,
	gitOpsOwner?: ResourceSummary["gitOpsOwner"],
	helmRelease?: string,
): ResourceSummary {
	return {
		cluster: "prod",
		apiVersion: kind === "Pod" || kind === "Service" ? "v1" : "apps/v1",
		kind,
		name,
		namespace: "catalog",
		age: "1m",
		health: "healthy",
		ownerRef,
		gitOpsOwner,
		helmRelease,
	};
}

const argoOwner = ownershipEdgeResource("Deployment", "catalog-argo-owner", undefined, {
	provider: "argo",
	kind: "Application",
	name: "catalog-app",
	namespace: "argocd",
	confidence: "metadata",
});
const fluxOwner = ownershipEdgeResource("Deployment", "catalog-flux-owner", undefined, {
	provider: "flux",
	kind: "Kustomization",
	name: "catalog-flux",
	namespace: "flux-system",
	confidence: "inventory",
});
const directFluxChild = ownershipEdgeResource(
	"Pod",
	"catalog-direct-flux-child",
	argoOwner.name,
	{
		provider: "flux",
		kind: "HelmRelease",
		name: "direct-flux-release",
		namespace: "flux-system",
		confidence: "inventory",
	},
);
const inheritedFluxChild = ownershipEdgeResource(
	"Pod",
	"catalog-inherited-flux-child",
	fluxOwner.name,
);
const ownerlessRow = ownershipEdgeResource("Service", "catalog-ownerless");
const missingParentRow = ownershipEdgeResource(
	"ConfigMap",
	"catalog-missing-parent-child",
	"not-present",
);
const cycleRows = [
	ownershipEdgeResource("ConfigMap", "catalog-cycle-a", "catalog-cycle-b"),
	ownershipEdgeResource("ConfigMap", "catalog-cycle-b", "catalog-cycle-a"),
];
const helmRow = ownershipEdgeResource(
	"Service",
	"catalog-helm-service",
	undefined,
	undefined,
	"catalog-release",
);
const ownershipEdgeRows = [
	argoOwner,
	fluxOwner,
	directFluxChild,
	inheritedFluxChild,
	ownerlessRow,
	missingParentRow,
	...cycleRows,
	helmRow,
];
const ownershipFixtureRows = [...ownershipRows, ...ownershipEdgeRows];
const ownershipFixtureSnapshot = JSON.stringify(ownershipFixtureRows);
const ownershipProjection = buildResourceTableProjection(ownershipFixtureRows);
const preferredArgoResourceKey = argoResourceIdentityKey(argoOwner);
if (!preferredArgoResourceKey) throw new Error("Missing preferred Argo identity");
const ownershipState: ResourceTableState = {
	search: "checkout",
	gitOpsFilter: "",
	healthFilter: "degraded",
	sort: { id: "memory", desc: true },
	pageIndex: 0,
	collapsedGroups: new Set(),
	selectedResource: ownershipRows[0],
};
const ownershipInteractionState: ResourceTableState = {
	...ownershipState,
	search: "",
	healthFilter: "all",
	preferredGitOpsResourceKeys: new Set([preferredArgoResourceKey]),
	selectedResource: argoOwner,
};
const ownershipFixtureModel = buildResourceTableModel(
	ownershipProjection,
	ownershipState,
);
const ownershipInteractionModel = buildResourceTableModel(
	ownershipProjection,
	ownershipInteractionState,
);
const ownershipProjectedRows = ownershipProjection.searchIndex.map(
	(entry) => entry.resource,
);
const expectedOwnershipNames = ownershipFixtureRows.map((row) => row.name);
const ownershipGroupLabels = new Set(
	ownershipInteractionModel.displayRows.map(gitOpsOwnershipGroupLabel),
);
const expectedInteractionNames = ownershipInteractionModel.scopedRows.map(
	(row) => row.name,
);
const displayedInteractionNames = ownershipInteractionModel.displayRows.map(
	(row) => row.name,
);
const ownedOrderSnapshot = ownershipProjectedRows.map((row) => row.name);

assert.equal(ownershipRows.length, 10_000);
assert.equal(rows.length, 10_000);
assert(
	kindRows.some(
		(row) => row.kind === "Widget" && row.apiVersion === widgetKind.apiVersion,
	),
);
assert(
	filterResourcesByKinds(kindRows, selectedKinds).every(
		(row) => row.kind !== "Widget" || row.apiVersion === widgetKind.apiVersion,
	),
);
const earlyKindRows = filterResourcesByKinds(kindRows, selectedKinds);
const lateKindRowsResult = filterResourcesByKinds(lateKindRows, selectedKinds);
const noMatchingKindRows = filterResourcesByKinds(kindRows, [unsupportedWidgetKind]);
assert.equal(earlyKindRows[0], rows.find((row) => row.kind === "Pod"));
assert.equal(lateKindRowsResult.at(-1), widgetRow);
assert.equal(noMatchingKindRows.length, 0);
assert.equal(ownershipRows.filter((row) => row.ownerRef).length, 7_500);
assert.equal(ownershipFixtureRows.length, ownershipRows.length + ownershipEdgeRows.length);
assert.equal(ownershipProjectedRows.length, ownershipFixtureRows.length);
assert.deepEqual(ownedOrderSnapshot, expectedOwnershipNames);
assert.equal(JSON.stringify(ownershipFixtureRows), ownershipFixtureSnapshot);
assert.equal(
	ownershipProjectedRows.find((row) => row.name === directFluxChild.name)
		?.gitOpsOwner?.name,
	"direct-flux-release",
);
assert.equal(
	ownershipProjectedRows.find((row) => row.name === inheritedFluxChild.name)
		?.gitOpsOwner?.name,
	"catalog-flux",
);
assert.equal(
	ownershipProjectedRows.find((row) => row.name === ownerlessRow.name)
		?.gitOpsOwner,
	undefined,
);
assert.equal(
	ownershipProjectedRows.find((row) => row.name === missingParentRow.name)
		?.gitOpsOwner,
	undefined,
);
assert(
	cycleRows.every(
		(row) =>
			ownershipProjectedRows.find((projected) => projected.name === row.name)
				?.gitOpsOwner === undefined,
	),
);
assert.equal(
	ownershipInteractionModel.displayRows.length,
	ownershipInteractionModel.filteredRows.length,
);
assert.deepEqual(
	[...displayedInteractionNames].sort(),
	[...expectedInteractionNames].sort(),
);
assert.equal(ownershipInteractionModel.displayRows[0], argoOwner);
assert(ownershipGroupLabels.has(gitOpsOwnershipGroupLabel(fluxOwner)));
assert(ownershipGroupLabels.has(gitOpsOwnershipGroupLabel(helmRow)));
assert(ownershipGroupLabels.has(gitOpsOwnershipGroupLabel(ownerlessRow)));
assert(ownershipFixtureModel.scopedRows.length > ownershipFixtureModel.filteredRows.length);
assert(
	ownershipFixtureModel.filteredRows.some(
		(row) => row.kind === "Pod" && row.gitOpsOwner?.name === "checkout-0",
	),
);

describe("F01 F02 F03 resource arrival and retained table state (10k rows)", () => {
	bench("buildResourceSearchIndex", () => {
		buildResourceSearchIndex(rows);
	});

	bench("filterResourceSearchIndex (search + gitops owner)", () => {
		filterResourceSearchIndex(searchIndex, "checkout-42", gitOpsFilter);
	});

	bench("buildResourceTableModel (search + health + sort + groups)", () => {
		buildResourceTableModel(projection, state);
	});

	bench("buildResourceTableModel (gitops owner filtered)", () => {
		buildResourceTableModel(projection, gitOpsState);
	});

	bench("F01 multi-hop ownership projection on arrival", () => {
		buildResourceTableProjection(ownershipFixtureRows);
	});

	bench("F03 kind filter + F01 projection for versioned CRDs", () => {
		buildResourceTableProjection(
			filterResourcesByKinds(lateKindRows, selectedKinds),
		);
	});

	bench("buildResourceTableModel repeated variants (reused projection)", () => {
		for (const variant of stateVariants)
			buildResourceTableModel(projection, variant);
	});

	bench("F02 retained model on multi-hop ownership projection", () => {
		buildResourceTableModel(ownershipProjection, ownershipState);
		buildResourceTableModel(ownershipProjection, ownershipInteractionState);
	});
});

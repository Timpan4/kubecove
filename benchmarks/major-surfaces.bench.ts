import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import type { GitOpsData } from "@/features/gitops/surfaceModel";
import {
	buildGitOpsRailItems,
	buildGitOpsSelections,
	buildGitOpsTable,
	gitOpsSelectionKey,
	gitOpsSelectionRevisionTooltipLines,
	gitOpsSelectionRevisionTooltipRows,
	gitOpsSelectionSourceLine,
	gitOpsSelectionSourceTooltipGroups,
	gitOpsSelectionSourceTooltipLines,
	gitOpsSelectionSourceTooltipTitle,
} from "@/features/gitops/surfaceModel";
import { gitOpsSelectionCells } from "@/features/gitops/surfaceSelection";
import { buildGitOpsReadState } from "@/features/gitops/surfaceState";
import { buildGitOpsSummary } from "@/features/gitops/surfaceSummary";
import { sortHelmReconciliationResources } from "@/features/helm/helpers";
import { buildHelmReleaseState } from "@/features/helm/surfaceState";
import type { TreeNodeId } from "@/lib/tree-nav";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	HealthAssessment,
	HelmReconciliationResource,
	HelmReleaseSummary,
} from "@/lib/types";

function fluxHealthAssessment(readyStatus: "False" | "True"): HealthAssessment {
	const state = readyStatus === "True" ? "healthy" : "degraded";
	return {
		state,
		completeness: "complete",
		winningSources: ["kubernetes"],
		reasons: [`Ready=${readyStatus}`],
		evidence: [
			{
				source: "kubernetes",
				raw: { type: "Ready", status: readyStatus },
				state,
				current: true,
				reason: `Ready=${readyStatus}`,
			},
		],
	};
}

const argoApps: ArgoApplicationSummary[] = Array.from(
	{ length: 2_000 },
	(_, index) => ({
		name: `application-${index}`,
		cluster: "prod",
		namespace: "argocd",
		project: `project-${index % 250}`,
		syncStatus: index % 5 === 0 ? "OutOfSync" : "Synced",
		healthStatus: index % 11 === 0 ? "Degraded" : "Healthy",
		healthAssessment: {
			...fluxHealthAssessment(index % 11 === 0 ? "False" : "True"),
			completeness: index % 13 === 0 ? "partial" : "complete",
		},
		destinationNamespace: `namespace-${index % 100}`,
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/platform.git",
		sourceRevision: `revision-${index}`,
		sources: [
			{
				repoUrl: "https://example.com/platform.git",
				path: `apps/service-${index % 100}`,
				chart: null,
				targetRevision: "main",
				resolvedRevision: `${String(index).padStart(8, "0")}abcdef`,
				reference: null,
				sourceMode:
					index % 3 === 0 ? "git" : index % 3 === 1 ? "helm" : "plugin",
			},
			{
				repoUrl: "https://example.com/overlays.git",
				path: `clusters/prod/${index % 10}`,
				chart: null,
				targetRevision: "main",
				resolvedRevision: `fedcba${String(index).padStart(7, "0")}`,
				reference: null,
				sourceMode:
					index % 3 === 0 ? "git" : index % 3 === 1 ? "helm" : "unknown",
			},
		],
		resourceNamespaces: [`namespace-${index % 100}`],
		age: "1m",
	}),
);
const appSets: ArgoApplicationSetSummary[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		name: `application-set-${index}`,
		cluster: "prod",
		namespace: "argocd",
		age: "1m",
		project: `project-${index}`,
		status: "Healthy",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: `namespace-${index % 100}`,
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/platform.git",
		sourceRevision: "main",
	}),
);
for (const app of argoApps) {
	const source = app.sources?.[0];
	assert(source && app.sources);
	app.sources.push({ ...source });
}
const projects: ArgoAppProjectSummary[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		name: `project-${index}`,
		cluster: "prod",
		namespace: "argocd",
		age: "1m",
		description: `Project ${index}`,
		status: "Active",
	}),
);
const flux: FluxResourceSummary[] = Array.from(
	{ length: 2_500 },
	(_, index) => {
		const readyStatus = index % 9 === 0 ? "False" : "True";
		return {
			cluster: "prod",
			name: `kustomization-${index}`,
			namespace: `namespace-${index % 100}`,
			age: "1m",
			resourceKind: {
				group:
					index % 3 === 0
						? "kustomize.toolkit.fluxcd.io"
						: index % 3 === 1
							? "helm.toolkit.fluxcd.io"
							: "source.toolkit.fluxcd.io",
				version: "v1",
				apiVersion: `${index % 3 === 0 ? "kustomize" : index % 3 === 1 ? "helm" : "source"}.toolkit.fluxcd.io/v1`,
				kind:
					index % 3 === 0
						? "Kustomization"
						: index % 3 === 1
							? "HelmRelease"
							: "GitRepository",
				plural:
					index % 3 === 0
						? "kustomizations"
						: index % 3 === 1
							? "helmreleases"
							: "gitrepositories",
				namespaced: true,
				category: "Kustomize",
			},
			healthAssessment: {
				...fluxHealthAssessment(readyStatus),
				completeness: index % 13 === 0 ? "partial" : "complete",
			},
			readyStatus,
			sourceKind: "GitRepository",
			sourceName: `repository-${index % 100}`,
			lastAppliedRevision: `revision-${index}`,
			message: index % 9 === 0 ? "Reconciliation failed" : "Applied",
			inventory: [],
		};
	},
);
const gitOpsData: GitOpsData = {
	argoDetected: true,
	apps: argoApps,
	appSets,
	projects,
	flux,
	fluxDetected: true,
};
const gitOpsReadSnapshots = {
	argoDetection: {
		data: true,
		isSuccess: true,
		isError: false,
		isPending: false,
		error: null,
	},
	fluxDetection: {
		data: { detected: true },
		isSuccess: true,
		isError: false,
		isPending: false,
		error: null,
	},
	argoApps: { data: argoApps, isPending: false, error: null },
	argoAppSets: { data: appSets, isPending: false, error: null },
	argoProjects: { data: projects, isPending: false, error: null },
	fluxResources: [{ data: flux, isPending: false, error: null }],
};
const argoNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:argo",
	kind: "Argo CD Applications",
};
const fluxNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:flux",
	kind: "Kustomization",
};
const appSetNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:argo",
	kind: "Argo CD ApplicationSets",
};
const projectsNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:argo",
	kind: "Argo CD AppProjects",
};
const selectedNodes: Array<{ node: TreeNodeId; rail: string; count: number }> =
	[
		{ node: argoNode, rail: "argo:applications", count: 2_000 },
		{ node: appSetNode, rail: "argo:applicationSets", count: 250 },
		{ node: projectsNode, rail: "argo:appProjects", count: 250 },
		{ node: fluxNode, rail: "flux:Kustomization", count: 834 },
		{
			node: { ...fluxNode, kind: "HelmRelease" },
			rail: "flux:HelmRelease",
			count: 833,
		},
		{
			node: { ...fluxNode, kind: "GitRepository" },
			rail: "flux:GitRepository",
			count: 833,
		},
	];
const detectedEmptySnapshots = {
	...gitOpsReadSnapshots,
	argoApps: { data: [], isPending: false, error: null },
	argoAppSets: { data: [], isPending: false, error: null },
	argoProjects: { data: [], isPending: false, error: null },
	fluxResources: [{ data: [], isPending: false, error: null }],
};
const emptyDetectedData = buildGitOpsReadState(detectedEmptySnapshots).query
	.data;
assert(emptyDetectedData);
assert.deepEqual(
	buildGitOpsSummary(emptyDetectedData, "argo:applications").detectedProviders,
	["Argo CD", "Flux"],
);
assert.equal(
	buildGitOpsSummary(emptyDetectedData, "argo:applications").totalObjects,
	0,
);
for (const { node, rail, count } of selectedNodes) {
	assert.equal(buildGitOpsTable(gitOpsData, node).rows.length, count);
	assert.equal(buildGitOpsSelections(gitOpsData, node).length, count);
	assert.equal(buildGitOpsSummary(gitOpsData, rail).totalObjects, 5_000);
	const first = buildGitOpsSelections(gitOpsData, node)[0];
	assert(first && gitOpsSelectionKey(first).includes(first.item.name));
}
const duplicateSourceSelection = buildGitOpsSelections(gitOpsData, argoNode)[0];
assert(duplicateSourceSelection);
assert.equal(
	gitOpsSelectionRevisionTooltipRows(duplicateSourceSelection).length,
	2,
);
assert.equal(
	gitOpsSelectionSourceTooltipGroups(duplicateSourceSelection).reduce(
		(count, group) => count + group.rows.length,
		0,
	),
	3,
);
assert(
	gitOpsSelectionSourceLine(duplicateSourceSelection)?.includes(
		"3 sources · apps/service-0 · clusters/prod/0",
	),
);

const helmReleases: HelmReleaseSummary[] = Array.from(
	{ length: 5_000 },
	(_, index) => ({
		cluster: "prod",
		name: `release-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		chart: `chart-${index % 50}`,
		status:
			index % 13 === 0
				? "failed"
				: index % 11 === 0
					? "pending-upgrade"
					: "deployed",
		storageKind: "Secret",
		storageName: `sh.helm.release.v1.release-${index}.v1`,
	}),
);
const reconciliationStatuses = [
	"tracked",
	"unlabeledLive",
	"missing",
	"labelOnly",
] as const;
const reconciliationResources: HelmReconciliationResource[] = Array.from(
	{ length: 10_000 },
	(_, index) => ({
		apiVersion: index % 2 === 0 ? "apps/v1" : "v1",
		kind: index % 2 === 0 ? "Deployment" : "Service",
		namespace: `namespace-${index % 100}`,
		name: `resource-${index}`,
		status: reconciliationStatuses[index % reconciliationStatuses.length],
		statusMessage: "Synthetic reconciliation result",
		inManifest: index % 4 !== 1,
		explicitHelmLabel: index % 4 !== 2,
	}),
);
assert.equal(
	argoApps.length + appSets.length + projects.length + flux.length,
	5_000,
);
assert.equal(helmReleases.length, 5_000);
assert.equal(reconciliationResources.length, 10_000);
assert(flux.some((resource) => resource.readyStatus === "False"));
assert(
	argoApps.some((resource) => resource.healthAssessment?.state === "degraded"),
);
assert(flux.some((resource) => resource.resourceKind.kind === "HelmRelease"));
assert(
	flux.some((resource) => resource.healthAssessment.completeness === "partial"),
);
assert(
	reconciliationStatuses.every((status) =>
		reconciliationResources.some((resource) => resource.status === status),
	),
);
const helmQueries = [
	{ search: "", namespace: null },
	{ search: "  DEPLOYED  ", namespace: "namespace-42" },
	{ search: "chart-42", namespace: null },
	{ search: "sh.helm.release.v1.release-42.v1", namespace: null },
	{ search: "release-42", namespace: "missing-namespace" },
] as const;
const allHelmState = buildHelmReleaseState(helmReleases, "");
assert.equal(allHelmState.filtered, helmReleases);
assert.equal(allHelmState.groups.length, 100);
assert.equal(
	allHelmState.groups.reduce(
		(count, group) => count + group.releases.length,
		0,
	),
	5_000,
);
assert.deepEqual(
	buildHelmReleaseState(helmReleases, "  DEPLOYED  ", "namespace-42").filtered,
	buildHelmReleaseState(helmReleases, "deployed", "namespace-42").filtered,
);
assert.equal(
	buildHelmReleaseState(helmReleases, helmQueries[3].search).filtered.length,
	1,
);
assert.equal(
	buildHelmReleaseState(helmReleases, "release-42", "missing-namespace")
		.filtered.length,
	0,
);

describe("major frontend surfaces", () => {
	bench(
		"F45 GitOps read state, rails, summaries, and selected tables for 5k objects",
		() => {
			const state = buildGitOpsReadState(gitOpsReadSnapshots);
			const data = state.query.data;
			if (!data) return;
			buildGitOpsRailItems(data);
			for (const { node, rail } of selectedNodes) {
				buildGitOpsSummary(data, rail);
				buildGitOpsTable(data, node);
				buildGitOpsSelections(data, node);
			}
			const emptyState = buildGitOpsReadState(detectedEmptySnapshots);
			if (emptyState.query.data) buildGitOpsRailItems(emptyState.query.data);
			const unavailableState = buildGitOpsReadState({
				...gitOpsReadSnapshots,
				argoDetection: { ...gitOpsReadSnapshots.argoDetection, data: false },
				fluxDetection: {
					...gitOpsReadSnapshots.fluxDetection,
					data: { detected: false },
				},
				argoApps: { ...gitOpsReadSnapshots.argoApps, data: [] },
				argoAppSets: { ...gitOpsReadSnapshots.argoAppSets, data: [] },
				argoProjects: { ...gitOpsReadSnapshots.argoProjects, data: [] },
				fluxResources: [{ data: [], isPending: false, error: null }],
			});
			if (unavailableState.query.data)
				buildGitOpsRailItems(unavailableState.query.data);
		},
	);

	bench(
		"F46 GitOps mounted row cells and source/revision tooltip preparation",
		() => {
			const selections = selectedNodes.flatMap(({ node }) =>
				buildGitOpsSelections(gitOpsData, node),
			);
			for (const selection of selections) {
				gitOpsSelectionKey(selection);
				gitOpsSelectionCells(selection);
				gitOpsSelectionSourceTooltipTitle(selection);
				gitOpsSelectionSourceTooltipGroups(selection);
				gitOpsSelectionSourceTooltipLines(selection);
				gitOpsSelectionRevisionTooltipRows(selection);
				gitOpsSelectionRevisionTooltipLines(selection);
			}
		},
	);

	bench(
		"F44 Helm release search, namespace groups, counts, and reconciliation sorting",
		() => {
			// F43 namespace grouping runs through the same buildHelmReleaseState caller path.
			const state = buildHelmReleaseState(
				helmReleases,
				"deployed",
				"namespace-42",
			);
			void state.filtered.length;
			void state.groups.length;
			for (const { search, namespace } of helmQueries)
				buildHelmReleaseState(helmReleases, search, namespace);
			void helmReleases.filter((release) => release.status === "deployed")
				.length;
			void helmReleases.filter((release) =>
				release.status?.startsWith("pending-"),
			).length;
			void helmReleases.filter((release) => release.status === "failed").length;
			sortHelmReconciliationResources(reconciliationResources);
		},
	);
});

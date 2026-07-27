import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	buildGitOpsRailItems,
	buildGitOpsTable,
} from "@/features/gitops/surfaceModel";
import type { GitOpsData } from "@/features/gitops/surfaceModel";
import { buildGitOpsSummary } from "@/features/gitops/surfaceSummary";
import {
	groupHelmReleasesByNamespace,
	resourcesOwnedByHelmRelease,
	sortHelmReconciliationResources,
} from "@/features/helm/helpers";
import {
	cockpitItems,
	filterCockpitItems,
} from "@/features/rbac/cockpitModel";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	HelmReconciliationResource,
	HelmReleaseSummary,
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	ResourceSummary,
	ServiceAccountSummary,
} from "@/lib/types";

function risks(index: number): RbacRiskIndicator[] {
	return index % 7 === 0
		? [{ level: "high", label: "Wildcard access", reason: "Synthetic risk" }]
		: [];
}

const serviceAccounts: ServiceAccountSummary[] = Array.from(
	{ length: 500 },
	(_, index) => ({
		cluster: "prod",
		name: `account-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		secretsCount: index % 3,
		imagePullSecretsCount: index % 2,
		risks: risks(index),
	}),
);
const roles: RbacRoleSummary[] = Array.from({ length: 500 }, (_, index) => ({
	cluster: "prod",
	kind: "Role",
	name: `role-${index}`,
	namespace: `namespace-${index % 100}`,
	age: "1m",
	rulesCount: 1,
	risks: risks(index),
	rules: [
		{
			verbs: index % 7 === 0 ? ["*"] : ["get", "list"],
			apiGroups: [""],
			resources: ["pods"],
			resourceNames: [],
			nonResourceUrls: [],
			risks: risks(index),
		},
	],
}));
const roleBindings: RbacBindingSummary[] = Array.from(
	{ length: 1_500 },
	(_, index) => {
		const accountIndex = index % serviceAccounts.length;
		return {
			cluster: "prod",
			kind: "RoleBinding",
			name: `binding-${index}`,
			namespace: `namespace-${accountIndex % 100}`,
			age: "1m",
			roleRefKind: "Role",
			roleRefName: `role-${accountIndex}`,
			subjects: [
				{
					kind: "ServiceAccount",
					name: `account-${accountIndex}`,
					namespace: `namespace-${accountIndex % 100}`,
				},
			],
			risks: risks(index),
		};
	},
);
const rbacData: RbacInspectionSummary = {
	cluster: "prod",
	warnings: [],
	coverage: [],
	serviceAccounts,
	roles,
	clusterRoles: [],
	roleBindings,
	clusterRoleBindings: [],
	namespaceAccess: [],
};

const argoApps: ArgoApplicationSummary[] = Array.from(
	{ length: 2_000 },
	(_, index) => ({
		name: `application-${index}`,
		cluster: "prod",
		namespace: "argocd",
		project: `project-${index % 250}`,
		syncStatus: index % 5 === 0 ? "OutOfSync" : "Synced",
		healthStatus: index % 11 === 0 ? "Degraded" : "Healthy",
		destinationNamespace: `namespace-${index % 100}`,
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/platform.git",
		sourceRevision: `revision-${index}`,
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
	(_, index) => ({
		cluster: "prod",
		name: `kustomization-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		resourceKind: {
			group: "kustomize.toolkit.fluxcd.io",
			version: "v1",
			apiVersion: "kustomize.toolkit.fluxcd.io/v1",
			kind: "Kustomization",
			plural: "kustomizations",
			namespaced: true,
			category: "Kustomize",
		},
		readyStatus: index % 9 === 0 ? "False" : "True",
		sourceKind: "GitRepository",
		sourceName: `repository-${index % 100}`,
		lastAppliedRevision: `revision-${index}`,
		message: index % 9 === 0 ? "Reconciliation failed" : "Applied",
		inventory: [],
	}),
);
const gitOpsData: GitOpsData = {
	argoDetected: true,
	apps: argoApps,
	appSets,
	projects,
	flux,
	fluxDetected: true,
};
const argoNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:argo",
	kind: "applications",
};
const fluxNode = {
	type: "kind" as const,
	section: "argo",
	group: "gitops:flux",
	kind: "Kustomization",
};

const helmReleases: HelmReleaseSummary[] = Array.from(
	{ length: 5_000 },
	(_, index) => ({
		cluster: "prod",
		name: `release-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		chart: `chart-${index % 50}`,
		status: index % 13 === 0 ? "failed" : "deployed",
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
const helmResources: ResourceSummary[] = Array.from(
	{ length: 10_000 },
	(_, index) => ({
		cluster: "prod",
		apiVersion: "v1",
		kind: "Pod",
		name: `pod-${index}`,
		namespace: `namespace-${index % 100}`,
		age: "1m",
		helmRelease: `release-${index % helmReleases.length}`,
	}),
);

assert.equal(serviceAccounts.length, 500);
assert.equal(roleBindings.length, 1_500);
assert.equal(argoApps.length + appSets.length + projects.length + flux.length, 5_000);
assert.equal(helmReleases.length, 5_000);
assert.equal(reconciliationResources.length, 10_000);
assert(rbacData.serviceAccounts.some((account) => account.risks.length > 0));
assert(flux.some((resource) => resource.readyStatus === "False"));
assert(
	reconciliationStatuses.every((status) =>
		reconciliationResources.some((resource) => resource.status === status),
	),
);

describe("major frontend surfaces", () => {
	bench("shape + filter RBAC service account cockpit", () => {
		const items = cockpitItems(rbacData, "Service Accounts");
		filterCockpitItems(items, "high", "account");
	});

	bench("build GitOps rails + summaries + tables for 5k objects", () => {
		buildGitOpsRailItems(gitOpsData);
		buildGitOpsSummary(gitOpsData, "argo:applications");
		buildGitOpsSummary(gitOpsData, "flux:Kustomization");
		buildGitOpsTable(gitOpsData, argoNode);
		buildGitOpsTable(gitOpsData, fluxNode);
	});

	bench("group + filter + sort Helm surface data", () => {
		groupHelmReleasesByNamespace(helmReleases);
		resourcesOwnedByHelmRelease(helmResources, helmReleases[42]);
		sortHelmReconciliationResources(reconciliationResources);
	});
});

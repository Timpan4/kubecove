import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import { applicationSpecFromYaml } from "@/features/gitops/argo-application-yaml";
import {
	argoDocumentText,
	highlightedArgoDiffSegments,
	resolveArgoResourceComparison,
} from "@/features/gitops/argo-resource-display";
import {
	argoComparisonDocument,
	argoReconciliationResources,
	argoResourceIdentityKey,
} from "@/features/gitops/argo-workspace-model";
import { buildArgoResourceDiff } from "@/features/resource-detail/yamlTabDiff";
import type { ArgoManagedResource, ArgoResourceComparison } from "@/lib/types";

const managedResources: ArgoManagedResource[] = Array.from(
	{ length: 2_500 },
	(_, index) => ({
		group: index % 3 === 0 ? "apps" : "",
		version: "v1",
		apiVersion: index % 3 === 0 ? "apps/v1" : "v1",
		kind: index % 2 === 0 ? "Deployment" : "Service",
		namespace: `namespace-${index % 100}`,
		name: `resource-${index}`,
		status: index % 5 === 0 ? "OutOfSync" : "Synced",
		health: index % 17 === 0 ? "Degraded" : "Healthy",
		requiresPruning: index % 19 === 0,
		targetState: {
			apiVersion: index % 3 === 0 ? "apps/v1" : "v1",
			kind: index % 2 === 0 ? "Deployment" : "Service",
			metadata: {
				name: `resource-${index}`,
				namespace: `namespace-${index % 100}`,
			},
			spec: {
				replicas: index % 5,
				selector: { matchLabels: { app: `resource-${index}` } },
			},
		},
		liveState: {
			apiVersion: index % 3 === 0 ? "apps/v1" : "v1",
			kind: index % 2 === 0 ? "Deployment" : "Service",
			metadata: {
				name: `resource-${index}`,
				namespace: `namespace-${index % 100}`,
			},
			spec: {
				replicas: index % 5 === 0 ? 9 : index % 5,
				selector: { matchLabels: { app: `resource-${index}` } },
			},
		},
	}),
);

const comparisons: ArgoResourceComparison[] = managedResources.map(
	(resource, index) => ({
		resource,
		targetState: resource.targetState,
		liveState: resource.liveState,
		normalizedLiveState: resource.liveState,
		modified: index % 5 === 0,
		exact: true,
		provenance: "connected-comparison",
		availableActions: [],
	}),
);
const selectedResource = managedResources[42];
assert(selectedResource);
const selectedResourceKey = argoResourceIdentityKey(selectedResource);
const selectedComparisonBase = comparisons[42];
assert(selectedComparisonBase);
const selectedComparison: ArgoResourceComparison = {
	...selectedComparisonBase,
	provenance: "selected-connected-comparison",
};
const expandedRemovalKeys = new Set(
	managedResources
		.filter((resource, index) => resource.requiresPruning && index % 2 === 0)
		.map((resource) => argoResourceIdentityKey(resource))
		.filter((key): key is string => key !== null),
);
const yamlDocuments = Array.from(
	{ length: 50 },
	(_, index) =>
		`apiVersion: argoproj.io/v1alpha1\nkind: Application\nmetadata:\n  name: app-${index}\nspec:\n  project: default\n  sources:\n    - repoURL: https://example.com/repo-${index}.git\n      path: apps/service\n      targetRevision: main\n    - repoURL: https://example.com/overlay-${index}.git\n      path: clusters/prod\n      targetRevision: release\n  destination:\n    namespace: tenant-${index % 10}\n`,
);
const malformedYaml = "apiVersion: [";
const invalidRootYaml = "[]";
const invalidSpecYaml =
	"apiVersion: argoproj.io/v1alpha1\nkind: Application\nspec: invalid\n";

assert.equal(managedResources.length, 2_500);
assert.equal(comparisons.length, managedResources.length);
assert(managedResources.some((resource) => resource.requiresPruning));
assert(expandedRemovalKeys.size > 0);
assert(managedResources.some((resource) => resource.requiresPruning &&
	!expandedRemovalKeys.has(argoResourceIdentityKey(resource) ?? "")));
assert.equal(applicationSpecFromYaml(malformedYaml).source, undefined);
assert.deepEqual(applicationSpecFromYaml(invalidRootYaml), {});
assert.deepEqual(applicationSpecFromYaml(invalidSpecYaml), {});
// SAFETY: yamlDocuments[0] is the synthetic Application fixture with two sources below.
assert.equal(
	(applicationSpecFromYaml(yamlDocuments[0]).sources as unknown[]).length,
	2,
);

describe("Argo resource display F48–F49", () => {
	bench(
		"F48 reconciliation display lookup, connected override, document text, and mounted diffs",
		() => {
			const visibleResources = argoReconciliationResources(managedResources);
			for (const resource of visibleResources) {
				const comparison = resolveArgoResourceComparison(
					resource,
					comparisons,
					"connected",
					selectedResourceKey,
					selectedComparison,
				);
				const documents = argoComparisonDocument(resource, comparison);
				if (
					resource.requiresPruning &&
					!expandedRemovalKeys.has(argoResourceIdentityKey(resource) ?? "")
				)
					continue;
				const desired = argoDocumentText(documents.desired);
				const normalizedLive = argoDocumentText(documents.normalizedLive);
				if (!desired || !normalizedLive) continue;
				for (const line of buildArgoResourceDiff(desired, normalizedLive)) {
					highlightedArgoDiffSegments(line);
				}
			}
		},
	);

	bench(
		"F49 parse each changed application YAML once and handle malformed fallback",
		() => {
			for (const document of yamlDocuments) applicationSpecFromYaml(document);
			applicationSpecFromYaml(malformedYaml);
			applicationSpecFromYaml(invalidRootYaml);
			applicationSpecFromYaml(invalidSpecYaml);
		},
	);
});

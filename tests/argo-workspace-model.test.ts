import { describe, expect, test } from "bun:test";
import {
	argoResourceCounts,
	argoResourceIdentityKey,
	argoResourceMatchesFilter,
	filterWorkspaceResourcesByArgo,
	type ArgoResourceFilter,
} from "../src/features/gitops/argo-workspace-model";
import type { ArgoManagedResource } from "../src/lib/gitops-types";

const managedResources: ArgoManagedResource[] = [
	{
		kind: "Deployment",
		namespace: "shop",
		name: "api",
		status: "OutOfSync",
		health: "Degraded",
		requiresPruning: true,
	},
	{
		kind: "Service",
		namespace: "shop",
		name: "api",
		status: "Synced",
		health: "Healthy",
	},
	{
		kind: "ConfigMap",
		namespace: "shop",
		name: "api-config",
		status: "OutOfSync",
		health: "Progressing",
	},
	{
		kind: "Job",
		namespace: "shop",
		name: "migrate",
		status: "Synced",
		health: "Healthy",
	},
	{
		kind: "Secret",
		namespace: "shop",
		name: "legacy",
		status: "OutOfSync",
		health: "Missing",
		requiresPruning: true,
	},
];

const workspaceResources = [
	...managedResources.map(({ kind, namespace, name }) => ({ kind, namespace, name })),
	{ kind: "Ingress", namespace: "shop", name: "unmanaged" },
];

describe("Argo managed-resource workspace model", () => {
	test("matches identities consistently by kind, namespace, and name", () => {
		expect(
			argoResourceIdentityKey({ kind: " Deployment ", namespace: "SHOP", name: "Api" }),
		).toBe("deployment:shop:api");
		expect(argoResourceIdentityKey({ kind: "Deployment", namespace: null, name: "api" })).toBe(
			"deployment::api",
		);
		expect(argoResourceIdentityKey({ kind: null, namespace: "shop", name: "api" })).toBeNull();
	});

	test("filters every Argo state independently", () => {
		const expected: Record<Exclude<ArgoResourceFilter, "none">, number> = {
			allManaged: 5,
			needsSync: 3,
			healthy: 2,
			degraded: 1,
			progressing: 1,
			prune: 2,
		};
		for (const [filter, count] of Object.entries(expected)) {
			expect(
				managedResources.filter((resource) =>
					argoResourceMatchesFilter(resource, filter as ArgoResourceFilter),
				),
			).toHaveLength(count);
		}
	});

	test("keeps health and prune counts separate from sync state", () => {
		expect(argoResourceCounts(managedResources)).toEqual({
			total: 5,
			needsSync: 3,
			current: 2,
			healthy: 2,
			degraded: 1,
			progressing: 1,
			prune: 2,
		});
		expect(argoResourceCounts(managedResources).needsSync + argoResourceCounts(managedResources).current).toBe(
			argoResourceCounts(managedResources).total,
		);
		expect(
			argoResourceCounts([{ kind: "Deployment", namespace: "shop", name: "unknown", status: "Unknown" }]),
		).toMatchObject({ total: 1, needsSync: 1, current: 0 });
		expect(
			argoResourceMatchesFilter(
				{ kind: "Deployment", namespace: "shop", name: "unknown", status: "Unknown" },
				"needsSync",
			),
		).toBe(true);
	});

	test("distinguishes All managed from Clear", () => {
		expect(
			filterWorkspaceResourcesByArgo(workspaceResources, managedResources, "allManaged"),
		).toHaveLength(5);
		expect(
			filterWorkspaceResourcesByArgo(workspaceResources, managedResources, "none"),
		).toHaveLength(6);
		expect(
			filterWorkspaceResourcesByArgo(workspaceResources, managedResources, "none"),
		).toContainEqual({ kind: "Ingress", namespace: "shop", name: "unmanaged" });
	});

	test("recomputes counts from refreshed post-sync data", () => {
		const refreshed = managedResources
			.filter((resource) => !resource.requiresPruning)
			.map((resource) => ({ ...resource, status: "Synced", health: "Healthy" }));
		expect(argoResourceCounts(refreshed)).toEqual({
			total: 3,
			needsSync: 0,
			current: 3,
			healthy: 3,
			degraded: 0,
			progressing: 0,
			prune: 0,
		});
	});
});

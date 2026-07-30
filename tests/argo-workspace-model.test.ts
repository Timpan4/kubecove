import { describe, expect, test } from "bun:test";
import {
	argoComparisonDocument,
	argoHistoryKey,
	argoReconciliationResources,
	argoResourceCounts,
	argoResourceIdentityKey,
	argoResourceMatchesFilter,
	argoSyncNeedsConfirmation,
	defaultArgoSyncSettings,
	filterWorkspaceResourcesByArgo,
	preserveArgoHistorySelection,
	preserveArgoResourceSelection,
	withArgoSyncSettings,
	type ArgoResourceFilter,
} from "../src/features/gitops/argo-workspace-model";
import type { ArgoApplicationHistory, ArgoManagedResource } from "../src/lib/gitops-types";

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
	test("matches identities consistently by API group, kind, namespace, and name", () => {
		expect(
			argoResourceIdentityKey({ group: " APPS ", kind: " Deployment ", namespace: "SHOP", name: "Api" }),
		).toBe("apps:deployment:shop:api");
		expect(argoResourceIdentityKey({ group: "", kind: "Deployment", namespace: null, name: "api" })).toBe(
			":deployment::api",
		);
		expect(argoResourceIdentityKey({ kind: null, namespace: "shop", name: "api" })).toBeNull();
	});

	test("does not collide resources from different API groups", () => {
		const managed = [{ group: "apps", kind: "Deployment", namespace: "shop", name: "api" }];
		const workspace = [
			{ apiVersion: "v1", kind: "Deployment", namespace: "shop", name: "api" },
			{ apiVersion: "apps/v1", kind: "Deployment", namespace: "shop", name: "api" },
		];

		expect(filterWorkspaceResourcesByArgo(workspace, managed, "allManaged")).toEqual([
			workspace[1],
		]);
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

describe("Argo briefing helpers", () => {
	const application = { name: "shop", namespace: "argocd" };

	test("groups all resources needing reconciliation, including removals", () => {
		expect(argoReconciliationResources(managedResources).map(({ name }) => name)).toEqual([
			"api",
			"api-config",
			"legacy",
		]);
		expect(
			argoReconciliationResources([
				{ kind: "ConfigMap", name: "remove", status: "Synced", health: "Healthy", requiresPruning: true },
			]),
		).toHaveLength(1);
	});

	test("keeps selection only while matching identity exists", () => {
		const selected = { group: "apps", kind: "Deployment", namespace: "shop", name: "api" };
		const refreshed = [{ ...selected, status: "Synced" }];
		expect(preserveArgoResourceSelection(selected, refreshed)).toBe(refreshed[0]);
		expect(preserveArgoResourceSelection(selected, [])).toBeNull();
		expect(preserveArgoResourceSelection(null, refreshed)).toBeNull();
	});

	test("keys history by application and falls back to newest entry", () => {
		const entries: ArgoApplicationHistory[] = [
			{ id: 3, revision: "new", revisions: [], sources: [] },
			{ revision: "old", revisions: [], sources: [] },
		];
		const selected = argoHistoryKey(application, entries[1]);
		expect(argoHistoryKey(application, entries[0])).toBe("argocd:shop:id:3");
		expect(preserveArgoHistorySelection(application, entries, selected)).toBe(selected);
		expect(preserveArgoHistorySelection(application, entries, "missing")).toBe("argocd:shop:id:3");
		expect(preserveArgoHistorySelection(application, [], selected)).toBeNull();
		expect(argoHistoryKey(application, { revisions: [], sources: [] })).toBe("argocd:shop:revision:unknown");
	});

	test("uses comparison data with managed-state fallbacks", () => {
		const resource = { kind: "Deployment", name: "api", targetState: { desired: 1 }, liveState: { live: 1 } };
		expect(argoComparisonDocument(resource)).toEqual({
			target: { desired: 1 }, desired: { desired: 1 }, live: { live: 1 }, normalizedLive: { live: 1 }, modified: undefined, exact: undefined, provenance: undefined,
		});
		expect(argoComparisonDocument(resource, {
			resource, targetState: { desired: 2 }, liveState: { live: 2 }, normalizedLiveState: { normalized: 2 }, modified: true, exact: true, provenance: "Argo", availableActions: [],
		})).toEqual({
			target: { desired: 2 }, desired: { desired: 2 }, live: { live: 2 }, normalizedLive: { normalized: 2 }, modified: true, exact: true, provenance: "Argo",
		});
	});

	test("defaults and applies application-wide sync settings", () => {
		const request = { transport: "connected" as const, application, action: "sync" as const, resources: managedResources };
		expect(defaultArgoSyncSettings).toEqual({ revision: "", prune: false, dryRun: false, force: false });
		expect(argoSyncNeedsConfirmation(defaultArgoSyncSettings)).toBe(false);
		for (const settings of [
			{ ...defaultArgoSyncSettings, revision: "main" },
			{ ...defaultArgoSyncSettings, prune: true },
			{ ...defaultArgoSyncSettings, dryRun: true },
			{ ...defaultArgoSyncSettings, force: true },
		]) expect(argoSyncNeedsConfirmation(settings)).toBe(true);
		const applicationDefaults = { ...defaultArgoSyncSettings, prune: true };
		expect(argoSyncNeedsConfirmation(applicationDefaults, applicationDefaults)).toBe(false);
		expect(
			argoSyncNeedsConfirmation({ ...applicationDefaults, prune: false }, applicationDefaults),
		).toBe(true);
		expect(withArgoSyncSettings(request, { revision: " main ", prune: true, dryRun: true, force: true })).toMatchObject({
			revision: "main", resources: [], prune: true, dryRun: true, force: true,
		});
	});
});

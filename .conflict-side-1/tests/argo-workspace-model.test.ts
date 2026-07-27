import { describe, expect, test } from "bun:test";
import {
	ARGO_SYNC_DEFAULTS,
	argoPhaseLabel,
	argoPrototypeResources,
	argoResourceCounts,
	createArgoPrototypeFixture,
	needsArgoSyncConfirmation,
} from "../src/features/gitops/argo-workspace-model";
import {
	argoWorkspaceSessionKey,
	createArgoWorkspaceSession,
	getArgoWorkspaceSession,
	patchArgoWorkspaceSession,
	resetArgoWorkspaceSession,
} from "../src/features/gitops/argo-workspace-session";
import type { ArgoApplicationSummary } from "../src/lib/gitops-types";

const app: ArgoApplicationSummary = {
	name: "shop",
	cluster: "mock-dev",
	namespace: "argocd",
	project: "platform",
	syncStatus: "OutOfSync",
	healthStatus: "Degraded",
	destinationNamespace: "shop",
	destinationServer: "https://kubernetes.default.svc",
	sourceRepo: "https://github.com/example/platform-config.git",
	sourceRevision: "main",
	resourceNamespaces: ["shop"],
	trackedResourceCount: 5,
	age: "12d",
};

describe("Argo frontend prototype model", () => {
	test("creates a deterministic Application fixture", () => {
		const first = createArgoPrototypeFixture(app);
		const second = createArgoPrototypeFixture(app);
		expect(second).toEqual(first);
		expect(first.resources.map((resource) => resource.health)).toContain("Degraded");
		expect(first.resources.some((resource) => resource.hook)).toBe(true);
		expect(first.resources.some((resource) => resource.requiresPruning)).toBe(true);
		expect(first.history).toHaveLength(2);
	});

	test("summarizes initial and synced resource state", () => {
		const fixture = createArgoPrototypeFixture(app);
		expect(argoResourceCounts(argoPrototypeResources(fixture, false))).toEqual({
			total: 5,
			outOfSync: 3,
			degraded: 1,
			progressing: 1,
			missing: 1,
			prune: 1,
		});
		expect(argoResourceCounts(argoPrototypeResources(fixture, true))).toEqual({
			total: 4,
			outOfSync: 0,
			degraded: 0,
			progressing: 0,
			missing: 0,
			prune: 0,
		});
	});

	test("confirms only options enabled beyond Application defaults", () => {
		expect(needsArgoSyncConfirmation({ ...ARGO_SYNC_DEFAULTS, prune: false })).toBe(false);
		expect(needsArgoSyncConfirmation({ ...ARGO_SYNC_DEFAULTS, dryRun: true })).toBe(false);
		expect(needsArgoSyncConfirmation({ ...ARGO_SYNC_DEFAULTS, force: true })).toBe(true);
		expect(needsArgoSyncConfirmation({ ...ARGO_SYNC_DEFAULTS, revision: "release-42" })).toBe(true);
	});

	test("models local progress labels and Application-scoped state", () => {
		expect(argoPhaseLabel("idle")).toBeNull();
		expect(argoPhaseLabel("refreshing")).toBe("Refreshing");
		expect(argoPhaseLabel("syncQueued")).toBe("Sync queued");
		expect(argoPhaseLabel("syncing")).toBe("Syncing");

		const firstKey = argoWorkspaceSessionKey("mock-dev", "workspace-a", "argocd", "shop");
		const secondKey = argoWorkspaceSessionKey("mock-dev", "workspace-a", "argocd", "payments");
		resetArgoWorkspaceSession(firstKey);
		resetArgoWorkspaceSession(secondKey);
		patchArgoWorkspaceSession(firstKey, { dashboardTab: "diff", phase: "syncing" });
		expect(getArgoWorkspaceSession(firstKey)).toMatchObject({ dashboardTab: "diff", phase: "syncing" });
		expect(getArgoWorkspaceSession(secondKey)).toEqual(createArgoWorkspaceSession());
	});
});

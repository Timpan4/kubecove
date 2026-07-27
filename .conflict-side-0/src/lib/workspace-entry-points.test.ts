import type { ResourceSummary } from "./types";
import {
	entryPointFromApplication,
	entryPointFromNamespace,
	entryPointFromResource,
	entryPointsEqual,
	isPinnedEntry,
	normalizeEntryPoints,
	reconcileEntryPoints,
	recordRecentEntry,
	resourceFromEntryPoint,
	togglePinnedEntry,
} from "./workspace-entry-points";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function resource(name: string): ResourceSummary {
	return {
		cluster: "admin@cluster",
		namespace: "default",
		kind: "Deployment",
		apiVersion: "apps/v1",
		name,
		age: "1h",
		health: "healthy",
	};
}

describe("workspace entry points", () => {
	test("toggles pinned resource identities without storing resource contents", () => {
		const entry = entryPointFromResource(resource("api"), "2026-07-13T10:00:00Z");
		const pinned = togglePinnedEntry(undefined, entry);

		expect(isPinnedEntry(pinned, entry)).toBe(true);
		expect(pinned.pinned[0]).toEqual({
			kind: "resource",
			clusterContext: "admin@cluster",
			namespace: "default",
			name: "api",
			resourceKind: "Deployment",
			apiVersion: "apps/v1",
			lastVisitedAt: "2026-07-13T10:00:00Z",
		});
		expect(togglePinnedEntry(pinned, entry).pinned).toEqual([]);
	});

	test("deduplicates and caps recent resource visits", () => {
		let entryPoints = normalizeEntryPoints();
		for (let index = 0; index < 10; index += 1) {
			entryPoints = recordRecentEntry(
				entryPoints,
				entryPointFromResource(resource(`api-${index}`), `2026-07-13T10:00:${index}Z`),
			);
		}
		entryPoints = recordRecentEntry(
			entryPoints,
			entryPointFromResource(resource("api-5"), "2026-07-13T11:00:00Z"),
		);

		expect(entryPoints.recent.length).toBe(8);
		expect(entryPoints.recent[0]?.name).toBe("api-5");
	});

	test("records namespace and application identities without live object data", () => {
		const namespace = entryPointFromNamespace(
			"admin@cluster",
			"payments",
			"2026-07-13T10:00:00Z",
		);
		const application = entryPointFromApplication(
			"admin@cluster",
			"checkout",
			"argocd",
			"2026-07-13T10:01:00Z",
		);

		expect(namespace).toEqual({
			kind: "namespace",
			clusterContext: "admin@cluster",
			namespace: "payments",
			name: "payments",
			lastVisitedAt: "2026-07-13T10:00:00Z",
		});
		expect(application).toEqual({
			kind: "app",
			clusterContext: "admin@cluster",
			namespace: "argocd",
			name: "checkout",
			resourceKind: "Application",
			apiVersion: "argoproj.io/v1alpha1",
			lastVisitedAt: "2026-07-13T10:01:00Z",
		});
	});

	test("reconciles only resources covered by the successful authoritative lookup", () => {
		const live = entryPointFromResource(resource("live"), "2026-07-13T10:00:00Z");
		const stale = entryPointFromResource(resource("stale"), "2026-07-13T09:00:00Z");
		const outOfScope = entryPointFromResource(
			{ ...resource("other"), namespace: "payments" },
			"2026-07-13T08:00:00Z",
		);
		const reconciled = reconcileEntryPoints(
			{ pinned: [live, stale, outOfScope], recent: [stale, live, outOfScope] },
			[resource("live")],
			[
				{
					clusterContext: "admin@cluster",
					requests: [{ kind: "Deployment", namespace: "default" }],
				},
			],
		);

		expect(reconciled.pinned).toEqual([live, outOfScope]);
		expect(reconciled.recent).toEqual([live, outOfScope]);
		expect(resourceFromEntryPoint(live)?.name).toBe("live");
		expect(entryPointsEqual(reconciled, { ...reconciled })).toBe(true);
	});
});

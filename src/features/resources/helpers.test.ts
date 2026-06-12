import type { ResourceSummary } from "@/lib/types";
import { resourceReadyChip, resourceStatusTone } from "./columns";
import {
	argoApplicationGitOpsFilterKey,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResourcesByHealth,
	filterResourceSearchIndex,
	uniqueGitOpsFilters,
} from "./helpers";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function resource(
	name: string,
	owner?: Partial<ResourceSummary>,
): ResourceSummary {
	return {
		kind: "Deployment",
		cluster: "kind-kind",
		name,
		namespace: "default",
		age: "1m",
		apiVersion: "apps/v1",
		health: "healthy",
		...owner,
	};
}

describe("resource GitOps filters", () => {
	test("keeps legacy Argo app filter values working", () => {
		const api = resource("api", { argoApp: "payments" });
		const worker = resource("worker", { argoApp: "jobs" });
		const rows = filterResourceSearchIndex(
			buildResourceSearchIndex([api, worker]),
			"",
			"payments",
		);

		expect(rows).toEqual([api]);
	});

	test("matches Argo Application GitOps filter keys against legacy app metadata", () => {
		const api = resource("api", { argoApp: "payments" });
		const worker = resource("worker", { argoApp: "jobs" });
		const rows = filterResourceSearchIndex(
			buildResourceSearchIndex([api, worker]),
			"",
			argoApplicationGitOpsFilterKey("payments"),
		);

		expect(rows).toEqual([api]);
	});

	test("filters Flux Kustomization and HelmRelease owners", () => {
		const api = resource("api", {
			gitOpsOwner: {
				provider: "flux",
				kind: "Kustomization",
				name: "apps",
				namespace: "flux-system",
				confidence: "label",
			},
		});
		const worker = resource("worker", {
			gitOpsOwner: {
				provider: "flux",
				kind: "HelmRelease",
				name: "worker",
				namespace: "default",
				confidence: "label",
			},
		});
		const rows = filterResourceSearchIndex(
			buildResourceSearchIndex([api, worker]),
			"",
			"flux:Kustomization:flux-system:apps",
		);

		expect(rows).toEqual([api]);
	});

	test("builds provider-specific GitOps filter labels", () => {
		const filters = uniqueGitOpsFilters([
			resource("api", { argoApp: "payments" }),
			resource("worker", {
				gitOpsOwner: {
					provider: "flux",
					kind: "HelmRelease",
					name: "worker",
					namespace: "default",
					confidence: "label",
				},
			}),
		]);

		expect(filters).toEqual([
			{ key: "payments", label: "Argo CD Application: payments" },
			{
				key: "flux:HelmRelease:default:worker",
				label: "Flux HelmRelease: default/worker",
			},
		]);
	});

	test("uses readable scope labels for Argo Application GitOps filters", () => {
		expect(
			describeResourceScope(
				[],
				["Pod"],
				argoApplicationGitOpsFilterKey("payments"),
			).at(-1),
		).toEqual({ kind: "gitOpsOwner", label: "GitOps", value: "payments" });
	});
});

describe("resource health helpers", () => {
	test("trusts backend health for completed job pods", () => {
		const pod = resource("descheduler-job-pod", {
			kind: "Pod",
			apiVersion: "v1",
			status: "Succeeded",
			ready: "False",
			health: "healthy",
		});

		const summary = buildResourceHealthSummary([pod]);

		expect(summary.healthy).toBe(1);
		expect(summary.degraded).toBe(0);
		expect(filterResourcesByHealth([pod], "degraded")).toEqual([]);
		expect(filterResourcesByHealth([pod], "healthy")).toEqual([pod]);
	});

	test("keeps restarted as tracked but outside unhealthy filters", () => {
		const pod = resource("api-0", {
			kind: "Pod",
			apiVersion: "v1",
			health: "restarted",
			restarts: 2,
		});

		const summary = buildResourceHealthSummary([pod]);

		expect(summary.restarted).toBe(1);
		expect(summary.untracked).toBe(0);
		expect(filterResourcesByHealth([pod], "unhealthy")).toEqual([]);
		expect(filterResourcesByHealth([pod], "restarted")).toEqual([pod]);
	});
});

describe("resource table status chips", () => {
	test("marks complete terminal status as success", () => {
		expect(resourceStatusTone("Complete")).toBe("success");
		expect(resourceStatusTone("Completed")).toBe("success");
		expect(resourceStatusTone("Succeeded")).toBe("success");
	});

	test("shows successful terminal pod readiness as completed success", () => {
		expect(
			resourceReadyChip(
				resource("job-pod", {
					kind: "Pod",
					status: "Succeeded",
					ready: "False",
				}),
			),
		).toEqual({ value: "Completed", variant: "success" });
	});

	test("keeps active not-ready pod readiness red", () => {
		expect(
			resourceReadyChip(
				resource("api-0", {
					kind: "Pod",
					status: "Running",
					ready: "False",
				}),
			),
		).toEqual({ value: "Not ready", variant: "error" });
	});
});

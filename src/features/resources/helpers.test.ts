import type { ResourceSummary } from "@/lib/types";
import { resourceReadyChip, resourceStatusTone } from "./columns";
import {
	argoApplicationGitOpsFilterKey,
	argoApplicationResourceNamespaces,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResourcesByHealth,
	filterResourceSearchIndex,
	formatResourceGroupLabel,
	hasResourceListGitOpsOwner,
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
			{ key: "payments", label: "Owned by Argo CD: payments" },
			{
				key: "flux:HelmRelease:default:worker",
				label: "Owned by Flux HelmRelease: default/worker",
			},
		]);
	});

	test("uses owned-by group labels for Argo resources", () => {
		expect(formatResourceGroupLabel(resource("api", { argoApp: "payments" }))).toBe(
			"Owned by Argo CD: payments",
		);
	});

	test("uses owned-by group labels for Flux Kustomizations", () => {
		expect(
			formatResourceGroupLabel(
				resource("api", {
					gitOpsOwner: {
						provider: "flux",
						kind: "Kustomization",
						name: "apps",
						namespace: "flux-system",
						confidence: "label",
					},
				}),
			),
		).toBe("Owned by Flux Kustomization: flux-system/apps");
	});

	test("does not use raw Kubernetes owner refs as list owners", () => {
		const pod = resource("api-795b", {
			kind: "Pod",
			ownerRef: "api-795b",
		});

		expect(hasResourceListGitOpsOwner(pod)).toBe(false);
		expect(formatResourceGroupLabel(pod)).toBe("Unmanaged resources");
	});

	test("keeps raw Kubernetes owner refs searchable when hidden from the list", () => {
		const pod = resource("api-0", {
			kind: "Pod",
			ownerRef: "api-795b",
		});

		const rows = filterResourceSearchIndex(
			buildResourceSearchIndex([pod]),
			"api-795b",
			"",
		);

		expect(rows).toEqual([pod]);
	});

	test("does not treat Flux source resources as list owners", () => {
		const sourceBacked = resource("source", {
			gitOpsOwner: {
				provider: "flux",
				kind: "GitRepository",
				name: "apps",
				namespace: "flux-system",
				confidence: "label",
			},
		});

		expect(hasResourceListGitOpsOwner(sourceBacked)).toBe(false);
		expect(uniqueGitOpsFilters([sourceBacked])).toEqual([]);
		expect(formatResourceGroupLabel(sourceBacked)).toBe("Unmanaged resources");
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

	test("opens tracked namespaces from Argo Application summaries", () => {
		expect(
			argoApplicationResourceNamespaces({
				resourceNamespaces: ["traefik", "monitoring", "traefik"],
				destinationNamespace: "default",
			}),
		).toEqual(["monitoring", "traefik"]);
	});

	test("falls back to destination namespace for Argo Application resources", () => {
		expect(
			argoApplicationResourceNamespaces({
				resourceNamespaces: [],
				destinationNamespace: "traefik",
			}),
		).toEqual(["traefik"]);
	});

	test("empty Argo Application namespace scope means all namespaces", () => {
		expect(
			argoApplicationResourceNamespaces({
				resourceNamespaces: [],
				destinationNamespace: null,
			}),
		).toEqual([]);
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

	test("does not double-count degraded resources with restarts", () => {
		const pod = resource("api-0", {
			kind: "Pod",
			apiVersion: "v1",
			health: "degraded",
			restarts: 2,
		});

		const summary = buildResourceHealthSummary([pod]);

		expect(summary.degraded).toBe(1);
		expect(summary.restarted).toBe(0);
		expect(filterResourcesByHealth([pod], "restarted")).toEqual([]);
	});
});

describe("resource table status chips", () => {
	test("marks complete terminal status as success", () => {
		expect(resourceStatusTone("Complete")).toBe("success");
		expect(resourceStatusTone("Completed")).toBe("success");
		expect(resourceStatusTone("Succeeded")).toBe("success");
		expect(resourceStatusTone("succeeded")).toBe("success");
	});

	test("normalizes warning and failure statuses", () => {
		expect(resourceStatusTone("Unknown")).toBe("warning");
		expect(resourceStatusTone("CrashLoopBackOff")).toBe("error");
		expect(resourceStatusTone("ImagePullBackOff")).toBe("error");
	});

	test("shows successful terminal pod readiness as completed success", () => {
		expect(
			resourceReadyChip(
				resource("job-pod", {
					kind: "Pod",
					status: "Succeeded",
					ready: "false",
				}),
			),
		).toEqual({ value: "Completed", variant: "success" });
	});

	test("normalizes ready booleans", () => {
		expect(
			resourceReadyChip(
				resource("api-0", {
					kind: "Pod",
					status: "Running",
					ready: "true",
				}),
			),
		).toEqual({ value: "Ready", variant: "success" });
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

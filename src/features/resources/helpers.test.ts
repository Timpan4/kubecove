import type { ResourceSummary } from "@/lib/types";
import {
	buildResourceSearchIndex,
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
});

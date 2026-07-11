import { describe, expect, test } from "bun:test";
import { buildGitOpsReadState } from "../src/features/gitops/surfaceState";

const readyDetection = {
	isSuccess: true,
	isError: false,
	isPending: false,
	error: null,
};
const emptyList = { data: [], isPending: false, error: null };

describe("GitOps surface state", () => {
	test("waits for both provider probes before exposing data", () => {
		const state = buildGitOpsReadState({
			argoDetection: { ...readyDetection, data: true },
			fluxDetection: { ...readyDetection, data: undefined, isSuccess: false, isPending: true },
			argoApps: emptyList,
			argoAppSets: emptyList,
			argoProjects: emptyList,
			fluxResources: [],
		});

		expect(state.query.data).toBeUndefined();
		expect(state.query.isPending).toBe(true);
	});

	test("keeps partial provider data visible when one list fails", () => {
		const listError = new Error("ApplicationSets unavailable");
		const state = buildGitOpsReadState({
			argoDetection: { ...readyDetection, data: true },
			fluxDetection: { ...readyDetection, data: { detected: false } },
			argoApps: {
				data: [{
					name: "api",
					cluster: "kind-dev",
					namespace: "argocd",
					project: "default",
					syncStatus: "Synced",
					healthStatus: "Healthy",
					destinationNamespace: "default",
					destinationServer: null,
					sourceRepo: null,
					sourceRevision: null,
					resourceNamespaces: ["default"],
					age: "1h",
				}],
				isPending: false,
				error: null,
			},
			argoAppSets: { data: [], isPending: false, error: listError },
			argoProjects: emptyList,
			fluxResources: [],
		});

		expect(state.listError).toBe(listError);
		expect(state.query.isError).toBe(false);
		expect(state.query.data?.apps).toHaveLength(1);
	});
});

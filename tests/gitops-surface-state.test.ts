import { describe, expect, test } from "bun:test";
import { buildGitOpsReadState } from "../src/features/gitops/surfaceState";
import {
	argoApplicationResourceNavigation,
	resolveTargetGitOpsSelection,
} from "../src/features/gitops/surfaceSelection";
import { selectedGitOpsApplicationName } from "../src/features/gitops";

const readyDetection = {
	isSuccess: true,
	isError: false,
	isPending: false,
	error: null,
};
const emptyList = { data: [], isPending: false, error: null };

describe("GitOps surface state", () => {
	const application = {
		type: "argoApp" as const,
		item: {
			name: "checkout",
			cluster: "kind-dev",
			namespace: "argocd",
			project: "default",
			syncStatus: "Synced",
			healthStatus: "Healthy",
			destinationNamespace: "store",
			destinationServer: null,
			sourceRepo: null,
			sourceRevision: null,
			resourceNamespaces: ["store", "payments"],
			age: "1h",
		},
	};

	test("resolves shortcut targets and mirrors selected applications into path state", () => {
		const resolved = resolveTargetGitOpsSelection([application], "checkout", true);
		const missing = resolveTargetGitOpsSelection([application], "missing", true);

		expect(resolved).toEqual({ selection: application, shouldResolve: true });
		expect(missing).toEqual({ selection: null, shouldResolve: true });
		expect(selectedGitOpsApplicationName(resolved.selection)).toBe("checkout");
		expect(selectedGitOpsApplicationName(null)).toBeNull();
	});

	test("opens Argo resources without an owner filter and preserves focus", () => {
		expect(argoApplicationResourceNavigation(application)).toEqual({
			namespaces: ["payments", "store"],
			gitOpsFilter: "",
			healthFilter: "all",
			focusApplication: application.item,
		});
	});

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

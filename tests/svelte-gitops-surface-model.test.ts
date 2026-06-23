import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildGitOpsSelections,
	buildGitOpsTable,
	gitOpsSelectionKey,
	gitOpsUnavailableProvider,
} from "../src/app/svelte/gitOpsSurfaceModel";

const data = {
	apps: [
		{
			name: "api",
			cluster: "kind-dev",
			namespace: "argocd",
			project: "default",
			syncStatus: "Synced",
			healthStatus: "Healthy",
			destinationNamespace: "default",
			destinationServer: null,
			sourceRepo: "https://git.example/api",
			sourceRevision: "main",
			resourceNamespaces: ["default"],
			age: "1d",
		},
	],
	appSets: [
		{
			name: "tenants",
			cluster: "kind-dev",
			namespace: "argocd",
			age: "1d",
			project: "platform",
			status: "Ready",
			syncStatus: "Synced",
			healthStatus: "Healthy",
			destinationNamespace: "tenant-a",
			destinationServer: null,
			sourceRepo: "https://git.example/tenants",
			sourceRevision: "main",
		},
	],
	projects: [
		{
			name: "platform",
			cluster: "kind-dev",
			namespace: "argocd",
			age: "2d",
			description: "Platform apps",
			status: "Active",
		},
	],
	flux: [
		{
			cluster: "kind-dev",
			name: "api",
			namespace: "flux-system",
			age: "1h",
			resourceKind: {
				group: "kustomize.toolkit.fluxcd.io",
				version: "v1",
				apiVersion: "kustomize.toolkit.fluxcd.io/v1",
				kind: "Kustomization",
				plural: "kustomizations",
				namespaced: true,
				category: "workloads",
			},
			readyStatus: "True",
			sourceName: "repo",
			lastAppliedRevision: "main@sha1:abc",
			message: "Applied",
			inventory: [],
		},
	],
};

function gitOpsSurfaceSource(): string {
	return [
		readFileSync(new URL("../src/app/svelte/AppSurfaces.svelte", import.meta.url), "utf8"),
		readFileSync(new URL("../src/app/svelte/GitOpsSurface.svelte", import.meta.url), "utf8"),
	].join("\n");
}

describe("svelte GitOps surface model", () => {
	test("selects Argo AppProject rows from sidebar kind", () => {
		expect(
			buildGitOpsTable(data, {
				type: "kind",
				section: "argo",
				group: "gitops:argo",
				kind: "Argo CD AppProjects",
			}),
		).toMatchObject({
			title: "Argo CD AppProjects",
			rows: [["platform", "argocd", "Active", "Platform apps", "2d"]],
		});
	});

	test("filters Flux rows by selected Flux kind", () => {
		expect(
			buildGitOpsTable(data, {
				type: "kind",
				section: "argo",
				group: "gitops:flux:workloads",
				kind: "Kustomization",
			}).rows,
		).toEqual([
			[
				"Kustomization/api",
				"flux-system",
				"True",
				"repo",
				"main@sha1:abc",
				"Applied",
			],
		]);
	});

	test("builds stable selectable row keys for GitOps detail drilldown", () => {
		const selections = buildGitOpsSelections(data, null);

		expect(selections.map(gitOpsSelectionKey)).toEqual([
			"argoApp:kind-dev:argocd:api",
		]);
	});

	test("defaults to Flux rows when Argo is absent but Flux is detected", () => {
		const fluxOnly = {
			...data,
			argoDetected: false,
			apps: [],
			appSets: [],
			projects: [],
			fluxDetected: true,
		};

		expect(buildGitOpsTable(fluxOnly, null)).toMatchObject({
			title: "Flux Resources",
			rows: [
				[
					"Kustomization/api",
					"flux-system",
					"True",
					"repo",
					"main@sha1:abc",
					"Applied",
				],
			],
		});
	});

	test("names selected unavailable GitOps providers", () => {
		const unavailable = {
			...data,
			argoDetected: false,
			fluxDetected: false,
			apps: [],
			appSets: [],
			projects: [],
			flux: [],
		};

		expect(
			gitOpsUnavailableProvider(unavailable, {
				type: "group",
				section: "argo",
				group: "gitops:argo",
			}),
		).toEqual({
			title: "Argo CD not detected",
			description: "Argo CD CRDs were not detected in this cluster.",
		});
		expect(
			gitOpsUnavailableProvider(unavailable, {
				type: "group",
				section: "argo",
				group: "gitops:flux",
			}),
		).toEqual({
			title: "Flux not detected",
			description: "Flux CRDs were not detected in this cluster.",
		});
	});

	test("does not silently cap GitOps or Flux rows in the Svelte surface", () => {
		const source = gitOpsSurfaceSource();

		expect(source).not.toContain("gitOpsSelections.slice");
		expect(source).not.toContain("data.flux.slice");
	});

	test("Svelte Argo Application details expose React parity incident fields", () => {
		const source = gitOpsSurfaceSource();

		expect(source).toContain("extractArgoStatusInsights");
		expect(source).toContain('selectedGitOpsItem.type === "argoApp"');
		expect(source).toContain("Sync & Health");
		expect(source).toContain("Unhealthy Resources");
		expect(source).toContain("Conditions");
		expect(source).toContain("Destination");
		expect(source).toContain("sourceRevision");
		expect(source).toContain("queryKeys.argoAppDetails(");
		expect(source).toContain("queryKeys.argoAppSetDetails(");
		expect(source).toContain("queryKeys.argoAppProjectDetails(");
		expect(source).toContain("queryKeys.fluxResourceDetails(");
		expect(source).not.toContain('"svelte-gitops-details"');
	});

	test("Svelte GitOps details show spinner while loading", () => {
		const source = gitOpsSurfaceSource();
		const loadingStart = source.indexOf("gitOpsDetailsQuery.isPending");
		const loadingEnd = source.indexOf("gitOpsDetailsQuery.isError", loadingStart);
		const loadingBody = source.slice(loadingStart, loadingEnd);

		expect(loadingBody).toContain('<Spinner class="size-4" />');
		expect(loadingBody).toContain("Loading GitOps details...");
	});

	test("Svelte GitOps probes providers before listing Argo CRDs", () => {
		const source = gitOpsSurfaceSource();

		expect(source).toContain("detectArgoCD");
		expect(source).toContain("queryKeys.argoDetect(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.fluxDetect(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.argoApps(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.argoAppSets(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("queryKeys.argoAppProjects(workspace.scope.clusterContext, kubeconfigSourceKey)");
		expect(source).toContain("createQueries(() => ({");
		expect(source).toContain("queryKeys.fluxResources(");
		expect(source).toContain("fluxResourceQueries.flatMap");
		expect(source).toContain("argoDetectionQuery.data === true");
		expect(source).toContain("No GitOps providers detected");
		expect(source).not.toContain('"svelte-gitops-surface"');
		expect(source).not.toContain('"svelte-flux-resources-surface"');
		expect(source).not.toContain(
			"const [apps, appSets, projects, fluxDetection] = await Promise.all([",
		);
	});

	test("Svelte GitOps keeps partial data visible when provider lists fail", () => {
		const source = gitOpsSurfaceSource();
		const queryStart = source.indexOf("const gitOpsQuery = $derived({");
		const queryEnd = source.indexOf("const selectedGitOpsItemKey", queryStart);
		const queryBody = source.slice(queryStart, queryEnd);

		expect(source).toContain("const gitOpsListError = $derived(");
		expect(source).toContain("Some GitOps resources could not load");
		expect(source).toContain("errorMessage(gitOpsListError)");
		expect(queryBody).toContain("isError: false");
		expect(queryBody).not.toContain("argoAppsQuery.isError");
		expect(queryBody).not.toContain("fluxResourceQueries.some((query) => query.isError)");
	});
});

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildGitOpsRailItems,
	buildGitOpsSelections,
	buildGitOpsTable,
	gitOpsDetailsActionKey,
	gitOpsSelectionAgeTooltip,
	gitOpsSelectionKey,
	gitOpsSelectionPrimaryAction,
	gitOpsSelectionResource,
	gitOpsSelectionRevisionLabel,
	gitOpsSelectionRevisionTooltipRows,
	gitOpsSelectionRevisionTooltipTitle,
	gitOpsSelectionRevisionTooltipLines,
	gitOpsSelectionSourceLabel,
	gitOpsSelectionSourceLine,
	gitOpsSelectionSourceMode,
	gitOpsSelectionSourceTooltip,
	gitOpsSelectionSourceTooltipGroups,
	gitOpsSelectionSourceTooltipLines,
	gitOpsSelectionSourceTooltipTitle,
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
			sourceMode: "git",
			sources: [
				{
					repoUrl: "https://git.example/api",
					targetRevision: "main",
					resolvedRevision: "abc123",
					path: "apps/api",
					chart: null,
					sourceMode: "git",
					reference: null,
				},
			],
			resourceNamespaces: ["default"],
			age: "1d",
			createdAt: "2026-06-23T10:00:00Z",
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
		readFileSync(new URL("../src/app/svelte/WorkspaceShell.svelte", import.meta.url), "utf8"),
		readFileSync(new URL("../src/app/svelte/AppSurfaces.svelte", import.meta.url), "utf8"),
		readFileSync(new URL("../src/app/svelte/GitOpsSurface.svelte", import.meta.url), "utf8"),
	].join("\n");
}

describe("svelte GitOps surface model", () => {
	test("builds provider rail counts for Argo CD and Flux groups", () => {
		expect(buildGitOpsRailItems(data)).toEqual([
			{
				key: "argo:applications",
				provider: "Argo CD",
				label: "Applications",
				count: 1,
				disabled: false,
			},
			{
				key: "argo:applicationSets",
				provider: "Argo CD",
				label: "ApplicationSets",
				count: 1,
				disabled: false,
			},
			{
				key: "argo:appProjects",
				provider: "Argo CD",
				label: "AppProjects",
				count: 1,
				disabled: false,
			},
			{
				key: "flux:Kustomization",
				provider: "Flux",
				label: "Kustomization",
				count: 1,
				disabled: false,
			},
		]);
	});

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

	test("filters selections for Applications, ApplicationSets, AppProjects, and Flux kinds", () => {
		expect(buildGitOpsSelections(data, null).map((item) => item.type)).toEqual(["argoApp"]);
		expect(
			buildGitOpsSelections(data, {
				type: "kind",
				section: "argo",
				group: "gitops:argo",
				kind: "Argo CD ApplicationSets",
			}).map((item) => item.type),
		).toEqual(["argoAppSet"]);
		expect(
			buildGitOpsSelections(data, {
				type: "kind",
				section: "argo",
				group: "gitops:argo",
				kind: "Argo CD AppProjects",
			}).map((item) => item.type),
		).toEqual(["argoProject"]);
		expect(
			buildGitOpsSelections(data, {
				type: "kind",
				section: "argo",
				group: "gitops:flux:workloads",
				kind: "Kustomization",
			}).map((item) => item.type),
		).toEqual(["flux"]);
	});

	test("builds stable selectable row keys for GitOps detail drilldown", () => {
		const selections = buildGitOpsSelections(data, null);

		expect(selections.map(gitOpsSelectionKey)).toEqual([
			"argoApp:kind-dev:argocd:api",
		]);
	});

	test("keeps Argo Application card primary action pointed at resources", () => {
		const [application] = buildGitOpsSelections(data, null);
		const [appSet] = buildGitOpsSelections(data, {
			type: "kind",
			section: "argo",
			group: "gitops:argo",
			kind: "Argo CD ApplicationSets",
		});

		expect(gitOpsSelectionPrimaryAction(application)).toBe("openResources");
		expect(gitOpsSelectionPrimaryAction(appSet)).toBe("details");
	});

	test("builds explicit details action selection keys", () => {
		const [application] = buildGitOpsSelections(data, null);

		expect(gitOpsDetailsActionKey(application)).toBe(
			"details:argoApp:kind-dev:argocd:api",
		);
	});

	test("summarizes GitOps source modes for card icons", () => {
		const [application] = buildGitOpsSelections(data, null);
		const multi = {
			...application,
			item: {
				...application.item,
				sourceRepo: "https://git.example/platform",
				sourceRevision: "2 revisions",
				sourceMode: "multi" as const,
				sourceCount: 4,
				sources: [
					{
						repoUrl: "https://git.example/platform",
						targetRevision: "main",
						resolvedRevision: "93cfd08b730c72d55ec119ea6cc64e4bbe6d975a",
						path: "solid-kubernetes",
						chart: null,
						sourceMode: "git",
						reference: null,
					},
					{
						repoUrl: "https://git.example/platform",
						targetRevision: "main",
						resolvedRevision: "93cfd08b730c72d55ec119ea6cc64e4bbe6d975a",
						path: "solid-kubernetes",
						chart: null,
						sourceMode: "git",
						reference: "values",
					},
					{
						repoUrl: "https://git.example/platform",
						targetRevision: "main",
						resolvedRevision: "93cfd08b730c72d55ec119ea6cc64e4bbe6d975a",
						path: "clusters/dev/cert-manager",
						chart: null,
						sourceMode: "git",
						reference: null,
					},
					{
						repoUrl: "https://charts.example",
						targetRevision: "v1.20.2",
						resolvedRevision: "v1.20.2",
						path: null,
						chart: "cert-manager",
						sourceMode: "helm",
						reference: null,
					},
				],
			},
		};
		const [project] = buildGitOpsSelections(data, {
			type: "kind",
			section: "argo",
			group: "gitops:argo",
			kind: "Argo CD AppProjects",
		});
		const [flux] = buildGitOpsSelections(data, {
			type: "kind",
			section: "argo",
			group: "gitops:flux:workloads",
			kind: "Kustomization",
		});

		expect(gitOpsSelectionSourceMode(application)).toBe("git");
		expect(gitOpsSelectionSourceMode(multi)).toBe("multi");
		expect(gitOpsSelectionSourceMode(project)).toBeNull();
		expect(gitOpsSelectionSourceMode(flux)).toBe("git");
		expect(gitOpsSelectionSourceLabel("multi", 4)).toBe("4 sources");
		expect(gitOpsSelectionSourceLine(application)).toBe(
			"https://git.example/api/apps/api",
		);
		expect(gitOpsSelectionRevisionLabel(application)).toBe("main");
		expect(gitOpsSelectionSourceLine(multi)).toBe(
			"4 sources · solid-kubernetes · clusters/dev/cert-manager · ...",
		);
		expect(gitOpsSelectionRevisionLabel(multi)).toBe("2 revisions");
		expect(gitOpsSelectionSourceTooltipTitle(multi)).toBe("Sources · 4 total");
		expect(gitOpsSelectionSourceTooltip(multi)).toContain("Sources · 4 total");
		expect(gitOpsSelectionSourceTooltip(multi)).not.toContain("1. ");
		expect(gitOpsSelectionSourceTooltipGroups(multi).map((group) => [group.label, group.rows.length])).toEqual([
			["Git", 3],
			["Helm", 1],
		]);
		expect(gitOpsSelectionSourceTooltipGroups(multi)[0]?.rows[0]?.fields).toEqual(
			expect.arrayContaining([
				{ label: "repo", value: "https://git.example/platform" },
				{ label: "path", value: "solid-kubernetes" },
				{ label: "target", value: "main" },
			]),
		);
		expect(gitOpsSelectionSourceTooltipLines(multi)).toContain(
			"Git: solid-kubernetes · repo https://git.example/platform · path solid-kubernetes · target main · resolved 93cfd08b730c72d55ec119ea6cc64e4bbe6d975a",
		);
		expect(gitOpsSelectionRevisionTooltipTitle(multi)).toBe("Revisions · 2 unique");
		expect(gitOpsSelectionRevisionTooltipRows(multi)).toEqual([
			{
				name: "solid-kubernetes, clusters/dev/cert-manager",
				fields: [
					{ label: "target", value: "main" },
					{ label: "resolved", value: "93cfd08" },
				],
			},
			{
				name: "cert-manager",
				fields: [
					{ label: "target", value: "v1.20.2" },
					{ label: "resolved", value: "v1.20.2" },
				],
			},
		]);
		expect(gitOpsSelectionRevisionTooltipLines(multi)).toEqual([
			"solid-kubernetes, clusters/dev/cert-manager: target main · resolved 93cfd08",
			"cert-manager: target v1.20.2 · resolved v1.20.2",
		]);
		expect(gitOpsSelectionAgeTooltip(application)).toBe("2026-06-23T10:00:00Z");
		expect(
			gitOpsSelectionAgeTooltip({
				...application,
				item: { ...application.item, createdAt: undefined },
			}),
		).toBeNull();
	});

	test("maps GitOps selections to dynamic resource inspector targets", () => {
		const [application] = buildGitOpsSelections(data, null);
		const [flux] = buildGitOpsSelections(data, {
			type: "kind",
			section: "argo",
			group: "gitops:flux:workloads",
			kind: "Kustomization",
		});

		expect(gitOpsSelectionResource(application)).toMatchObject({
			kind: "Application",
			cluster: "kind-dev",
			name: "api",
			namespace: "argocd",
			apiVersion: "argoproj.io/v1alpha1",
			group: "argoproj.io",
			version: "v1alpha1",
			plural: "applications",
			dynamic: true,
			health: "healthy",
			gitOpsOwner: { provider: "argo", kind: "Application", name: "api" },
		});
		expect(gitOpsSelectionResource(flux)).toMatchObject({
			kind: "Kustomization",
			cluster: "kind-dev",
			name: "api",
			namespace: "flux-system",
			apiVersion: "kustomize.toolkit.fluxcd.io/v1",
			plural: "kustomizations",
			dynamic: true,
			health: "healthy",
			gitOpsOwner: { provider: "flux", kind: "Kustomization", name: "api" },
		});
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

	test("Svelte GitOps details reuse the resource inspector", () => {
		const source = gitOpsSurfaceSource();

		expect(source).toContain("onResourceInspect");
		expect(source).toContain("gitOpsSelectionResource(selection)");
		expect(source).toContain("<ResourceDetailPanel");
		expect(source).toContain('sizeKey={resourceInspectorSizeKey}');
		expect(source).toContain('viewMode === "argo" ? 30 : 40');
		expect(source).toContain('viewMode === "argo" ? 25 : 33');
		expect(source).not.toContain("gitOpsDetailsQuery");
		expect(source).not.toContain("extractArgoStatusInsights");
		expect(source).not.toContain('"svelte-gitops-details"');
	});

	test("Svelte GitOps details stay explicit instead of hijacking card clicks", () => {
		const source = gitOpsSurfaceSource();

		expect(source).toContain('gitOpsSelectionPrimaryAction(selection) === "openResources"');
		expect(source).toContain("openSelectedArgoApplicationResources(selection)");
		expect(source).toContain("openGitOpsDetails(event: MouseEvent, selection: GitOpsSelection)");
		expect(source).toContain("data-details-key={gitOpsDetailsActionKey(item)}");
	});

	test("Svelte GitOps tooltips reset delay between targets", () => {
		const source = gitOpsSurfaceSource();

		expect(source).toContain("<TooltipProvider delayDuration={400} skipDelayDuration={0}>");
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
		expect(source).toContain("error={gitOpsListError}");
		expect(source).toContain('mode="compact"');
		expect(queryBody).toContain("isError: false");
		expect(queryBody).not.toContain("argoAppsQuery.isError");
		expect(queryBody).not.toContain("fluxResourceQueries.some((query) => query.isError)");
	});
});

import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceKind,
	FluxResourceSummary,
} from "@/lib/types";
import {
	buildGitOpsOverviewFilters,
	chooseDefaultGitOpsFilter,
	fluxFamilyInstalledKindCount,
	fluxKindLabelFromFilterKey,
	fluxResourceKindFromFilterKey,
} from "./gitops-overview-helpers";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function app(name: string): ArgoApplicationSummary {
	return {
		name,
		cluster: "kind-kind",
		namespace: "argocd",
		project: "default",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: "default",
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.test/repo.git",
		sourceRevision: "main",
		resourceNamespaces: ["default"],
		trackedResourceCount: 3,
		age: "1h",
	};
}

function appset(name: string): ArgoApplicationSetSummary {
	return {
		name,
		cluster: "kind-kind",
		namespace: "argocd",
		age: "1h",
		project: "default",
		status: "Ready",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: "default",
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.test/repo.git",
		sourceRevision: "main",
	};
}

function project(name: string): ArgoAppProjectSummary {
	return {
		name,
		cluster: "kind-kind",
		namespace: "argocd",
		age: "1h",
		description: null,
		status: "Ready",
	};
}

function fluxKind(kind: string, category = "source"): FluxResourceKind {
	return {
		group: `${category}.toolkit.fluxcd.io`,
		version: "v1",
		apiVersion: `${category}.toolkit.fluxcd.io/v1`,
		kind,
		plural: `${kind.toLowerCase()}s`,
		namespaced: true,
		category,
	};
}

function fluxRow(kind: FluxResourceKind, name: string): FluxResourceSummary {
	return {
		cluster: "kind-kind",
		name,
		namespace: "flux-system",
		age: "1h",
		resourceKind: kind,
		readyStatus: "True",
		suspended: false,
		sourceKind: "GitRepository",
		sourceName: "platform",
		lastAppliedRevision: "main@sha1:abc",
		inventory: [],
	};
}

describe("GitOps overview filters", () => {
	test("chooses first non-empty category in GitOps landing priority order", () => {
		const kustomization = fluxKind("Kustomization", "kustomize");
		const helmRelease = fluxKind("HelmRelease", "helm");
		const filters = buildGitOpsOverviewFilters({
			argoDetected: true,
			showUnavailableArgo: false,
			apps: [app("web")],
			appsets: [appset("generated")],
			projects: [project("default")],
			fluxDetected: true,
			showUnavailableFlux: false,
			fluxKinds: [kustomization, helmRelease],
			fluxRows: [
				fluxRow(kustomization, "platform"),
				fluxRow(helmRelease, "ingress"),
			],
		});

		expect(chooseDefaultGitOpsFilter(filters)).toBe("argo:applications");
	});

	test("falls through to Flux Kustomizations before Flux HelmReleases", () => {
		const kustomization = fluxKind("Kustomization", "kustomize");
		const helmRelease = fluxKind("HelmRelease", "helm");
		const filters = buildGitOpsOverviewFilters({
			argoDetected: true,
			showUnavailableArgo: false,
			apps: [],
			appsets: [appset("generated")],
			projects: [project("default")],
			fluxDetected: true,
			showUnavailableFlux: false,
			fluxKinds: [kustomization, helmRelease],
			fluxRows: [
				fluxRow(kustomization, "platform"),
				fluxRow(helmRelease, "ingress"),
			],
		});

		expect(chooseDefaultGitOpsFilter(filters)).toBe("flux:Kustomization");
	});

	test("counts Argo CD and Flux resources for the filter rail", () => {
		const gitRepository = fluxKind("GitRepository");
		const helmRepository = fluxKind("HelmRepository");
		const kustomization = fluxKind("Kustomization", "kustomize");
		const filters = buildGitOpsOverviewFilters({
			argoDetected: true,
			showUnavailableArgo: false,
			apps: [app("web"), app("api")],
			appsets: [appset("generated")],
			projects: [project("default")],
			fluxDetected: true,
			showUnavailableFlux: false,
			fluxKinds: [gitRepository, helmRepository, kustomization],
			fluxRows: [
				fluxRow(gitRepository, "platform"),
				fluxRow(gitRepository, "apps"),
				fluxRow(kustomization, "platform"),
			],
		});

		expect(filters.map((filter) => [filter.key, filter.count])).toEqual([
			["argo:applications", 2],
			["argo:applicationSets", 1],
			["argo:appProjects", 1],
			["flux:GitRepository", 2],
			["flux:HelmRepository", 0],
			["flux:Kustomization", 1],
		]);
		expect(fluxFamilyInstalledKindCount(filtersToFluxKinds(filters), "sources")).toBe(
			2,
		);
	});

	test("shows unavailable provider rows as disabled placeholders when enabled", () => {
		const filters = buildGitOpsOverviewFilters({
			argoDetected: false,
			showUnavailableArgo: true,
			apps: [],
			appsets: [],
			projects: [],
			fluxDetected: false,
			showUnavailableFlux: true,
			fluxKinds: [],
			fluxRows: [],
		});

		expect(filters.find((filter) => filter.key === "argo:applications")?.disabled).toBe(
			true,
		);
		expect(filters.find((filter) => filter.key === "flux:GitRepository")?.disabled).toBe(
			true,
		);
		expect(chooseDefaultGitOpsFilter(filters)).toBe(null);
	});

	test("maps Flux filter keys back to polished labels and installed kinds", () => {
		const gitRepository = fluxKind("GitRepository");
		const helmRelease = fluxKind("HelmRelease", "helm");

		expect(fluxKindLabelFromFilterKey("flux:GitRepository")).toBe(
			"Flux Git Repositories",
		);
		expect(
			fluxResourceKindFromFilterKey("flux:HelmRelease", [
				gitRepository,
				helmRelease,
			])?.kind,
		).toBe("HelmRelease");
	});
});

function filtersToFluxKinds(filters: { key: string }[]): FluxResourceKind[] {
	return filters
		.filter((filter) => filter.key.startsWith("flux:"))
		.map((filter) => fluxKind(filter.key.slice("flux:".length)));
}

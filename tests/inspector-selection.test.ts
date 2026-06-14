import { beforeEach, describe, expect, test } from "bun:test";
import { useDashboardStore } from "../src/lib/hooks";
import type {
	ArgoApplicationSummary,
	FluxResourceSummary,
	HelmReleaseSummary,
	ResourceSummary,
} from "../src/lib/types";

function resource(overrides: Partial<ResourceSummary> = {}): ResourceSummary {
	return {
		kind: "Pod",
		cluster: "ctx",
		name: "api",
		namespace: "default",
		age: "1d",
		health: "healthy",
		...overrides,
	};
}

function argoApp(
	overrides: Partial<ArgoApplicationSummary> = {},
): ArgoApplicationSummary {
	return {
		name: "shop",
		cluster: "ctx",
		namespace: "argocd",
		project: "default",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: "default",
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://example.com/repo.git",
		sourceRevision: "main",
		resourceNamespaces: ["default"],
		age: "1d",
		...overrides,
	};
}

function fluxResource(
	overrides: Partial<FluxResourceSummary> = {},
): FluxResourceSummary {
	return {
		cluster: "ctx",
		name: "shop",
		namespace: "flux-system",
		age: "1d",
		resourceKind: {
			group: "kustomize.toolkit.fluxcd.io",
			version: "v1",
			apiVersion: "kustomize.toolkit.fluxcd.io/v1",
			kind: "Kustomization",
			plural: "kustomizations",
			namespaced: true,
			category: "kustomize",
		},
		inventory: [],
		...overrides,
	};
}

function helmRelease(
	overrides: Partial<HelmReleaseSummary> = {},
): HelmReleaseSummary {
	return {
		cluster: "ctx",
		name: "shop",
		namespace: "default",
		age: "1d",
		storageKind: "Secret",
		storageName: "sh.helm.release.v1.shop.v1",
		...overrides,
	};
}

describe("inspector selection store", () => {
	beforeEach(() => {
		useDashboardStore.setState({
			selectedKinds: [],
			selection: null,
			argoDetected: false,
			selectedArgoAppFilter: "",
			resourceInitialSearch: "",
			resourceHealthFilter: "all",
			viewMode: "resources",
			selectedTreeNode: null,
			expandedSections: [],
		});
	});

	test("openView clears an existing inspector selection", () => {
		useDashboardStore.getState().select({
			type: "helm",
			release: helmRelease(),
		});

		useDashboardStore.getState().openView("resources");

		expect(useDashboardStore.getState().selection).toBeNull();
		expect(useDashboardStore.getState().viewMode).toBe("resources");
	});

	test("flux selection round-trips through the union", () => {
		const selected = fluxResource({ name: "worker" });

		useDashboardStore.getState().select({ type: "flux", resource: selected });

		expect(useDashboardStore.getState().selection).toEqual({
			type: "flux",
			resource: selected,
		});
	});

	test("openView sets GitOps filter and resets resource filters by default", () => {
		useDashboardStore.setState({
			resourceInitialSearch: "api",
			resourceHealthFilter: "degraded",
		});

		useDashboardStore
			.getState()
			.openView("argo", { argoAppFilter: "shop" });

		expect(useDashboardStore.getState().viewMode).toBe("argo");
		expect(useDashboardStore.getState().selectedArgoAppFilter).toBe("shop");
		expect(useDashboardStore.getState().resourceInitialSearch).toBe("");
		expect(useDashboardStore.getState().resourceHealthFilter).toBe("all");
	});

	test("resource filters are set for resources and reset on later view transitions", () => {
		useDashboardStore.getState().openView("resources", {
			initialSearch: "api",
			healthFilter: "unhealthy",
		});

		expect(useDashboardStore.getState().resourceInitialSearch).toBe("api");
		expect(useDashboardStore.getState().resourceHealthFilter).toBe("unhealthy");

		useDashboardStore.getState().openView("incidents");

		expect(useDashboardStore.getState().resourceInitialSearch).toBe("");
		expect(useDashboardStore.getState().resourceHealthFilter).toBe("all");
	});

	test("all selection variants round-trip", () => {
		const variants = [
			{ type: "resource" as const, resource: resource() },
			{ type: "argo" as const, app: argoApp() },
			{ type: "flux" as const, resource: fluxResource() },
			{ type: "helm" as const, release: helmRelease() },
		];

		for (const variant of variants) {
			useDashboardStore.getState().select(variant);
			expect(useDashboardStore.getState().selection?.type).toBe(variant.type);
		}
	});

	test("openView preserves or clears tree node based on explicit option", () => {
		const node = { type: "section" as const, section: "workloads" };
		useDashboardStore.setState({ selectedTreeNode: node });

		useDashboardStore.getState().openView("resources");
		expect(useDashboardStore.getState().selectedTreeNode).toEqual(node);

		useDashboardStore.getState().openView("resources", { treeNode: null });
		expect(useDashboardStore.getState().selectedTreeNode).toBeNull();
	});

	test("view-only settings transitions preserve selection and resource filters", () => {
		useDashboardStore.setState({
			selection: { type: "resource", resource: resource() },
			selectedArgoAppFilter: "shop",
			resourceInitialSearch: "api",
			resourceHealthFilter: "attention",
		});

		useDashboardStore.getState().setViewMode("settings");
		useDashboardStore.getState().setViewMode("resources");

		expect(useDashboardStore.getState().selection?.type).toBe("resource");
		expect(useDashboardStore.getState().selectedArgoAppFilter).toBe("shop");
		expect(useDashboardStore.getState().resourceInitialSearch).toBe("api");
		expect(useDashboardStore.getState().resourceHealthFilter).toBe("attention");
	});
});

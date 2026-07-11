import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	findHelmReleaseTarget,
	helmReconciliationResourceLabel,
	helmReconciliationStatusLabel,
	helmReconciliationStatusTone,
	sortHelmReconciliationResources,
} from "../src/features/helm/helpers";
import type {
	HelmReconciliationResource,
	HelmReleaseSummary,
} from "../src/lib/types";
import { createWorkspaceRecord } from "../src/lib/workspace-model";
import {
	createWorkspaceNavigation,
	navigateWorkspace,
} from "../src/app/svelte/workspaceNavigation";

function row(
	kind: string,
	name: string,
	namespace: string | undefined,
	status: HelmReconciliationResource["status"],
): HelmReconciliationResource {
	return {
		apiVersion: kind === "Deployment" ? "apps/v1" : "v1",
		kind,
		name,
		namespace,
		status,
		statusMessage: status,
		inManifest: status !== "labelOnly",
		explicitHelmLabel: status === "tracked" || status === "labelOnly",
	};
}

function release(name: string, namespace: string): HelmReleaseSummary {
	return {
		cluster: "kind-dev",
		name,
		namespace,
		storageKind: "Secret",
		storageName: `sh.helm.release.v1.${name}.v1`,
		age: "1d",
	};
}

describe("Helm reconciliation UI helpers", () => {
	test("labels and tones classify reconciliation statuses", () => {
		expect(helmReconciliationStatusLabel("tracked")).toBe("Tracked");
		expect(helmReconciliationStatusTone("tracked")).toBe("success");
		expect(helmReconciliationStatusLabel("unlabeledLive")).toBe(
			"Unlabeled live",
		);
		expect(helmReconciliationStatusTone("unlabeledLive")).toBe("warning");
		expect(helmReconciliationStatusLabel("missing")).toBe("Missing");
		expect(helmReconciliationStatusTone("missing")).toBe("error");
		expect(helmReconciliationStatusLabel("labelOnly")).toBe("Label-only");
		expect(helmReconciliationStatusTone("labelOnly")).toBe("warning");
		expect(helmReconciliationStatusLabel("unavailable")).toBe("Unavailable");
		expect(helmReconciliationStatusTone("unavailable")).toBe("neutral");
	});

	test("sorts resources by namespace kind name and keeps readable labels", () => {
		const rows = [
			row("Service", "api", "payments", "tracked"),
			row("Deployment", "api", "payments", "missing"),
			row("Secret", "cluster-extra", undefined, "labelOnly"),
		];

		const sorted = sortHelmReconciliationResources(rows);

		expect(sorted.map(helmReconciliationResourceLabel)).toEqual([
			"Secret/cluster-extra",
			"Deployment/api",
			"Service/api",
		]);
	});

	test("finds targeted Helm releases from resource detail handoff", () => {
		const releases = [release("api", "prod"), release("api", "dev")];

		expect(findHelmReleaseTarget(releases, { name: "api", namespace: "dev" })).toBe(
			releases[1],
		);
		expect(findHelmReleaseTarget(releases, { name: "api" })).toBe(releases[0]);
		expect(findHelmReleaseTarget(releases, { name: "missing" })).toBeNull();
	});

	test("Svelte resource detail can hand Helm releases to the Helm surface", () => {
		const detail = readFileSync("src/features/resource-detail/DetailsTab.svelte", "utf8");
		const shell = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const surfaces = readFileSync("src/app/svelte/AppSurfaces.svelte", "utf8");

		expect(detail).toContain("Open Helm release");
		expect(shell).toContain("onOpenHelmRelease={openHelmReleaseFromResource}");
		expect(shell).toContain("openHelmReleaseFromResource");
		expect(shell).toContain("{targetHelmRelease}");
		expect(surfaces).toContain("findHelmReleaseTarget");
		expect(surfaces).toContain("onTargetHelmReleaseResolved?.()");
	});

	test("Svelte Helm details open release-filtered resources", () => {
		const browser = readFileSync(
			"src/features/resources/ResourceBrowser.svelte",
			"utf8",
		);
		const surfaces = readFileSync("src/app/svelte/HelmSurface.svelte", "utf8");
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		const navigation = navigateWorkspace(createWorkspaceNavigation(workspace), {
			type: "openResources",
			namespaces: "payments",
			search: "checkout",
		});

		expect(browser).toContain('initialSearch = ""');
		expect(browser).toContain("search = pathState?.search ?? initialSearch");
		expect(navigation.resourceInitialSearch).toBe("checkout");
		expect(navigation.selectedNode).toEqual({
			type: "namespace",
			section: "namespaces",
			namespace: "payments",
		});
		expect(surfaces).toContain(
			"onOpenResources(selectedHelmRelease?.namespace, selectedHelmRelease?.name)",
		);
	});

	test("Svelte Helm details show all decoded manifest resources", () => {
		const surfaces = readFileSync("src/app/svelte/HelmSurface.svelte", "utf8");

		expect(surfaces).toContain('headers={["Kind", "Name", "Namespace", "API"]}');
		expect(surfaces).toContain("rows={details.manifestSummary.resources.map(");
		expect(surfaces).not.toContain("details.manifestSummary.resources.slice(");
	});

	test("Svelte Helm details show spinner while loading", () => {
		const surfaces = readFileSync("src/app/svelte/HelmSurface.svelte", "utf8");
		const loadingStart = surfaces.indexOf("helmDetailsQuery.isPending");
		const loadingEnd = surfaces.indexOf("helmDetailsQuery.isError", loadingStart);
		const loadingBody = surfaces.slice(loadingStart, loadingEnd);

		expect(loadingBody).toContain('<Spinner class="size-4" />');
		expect(loadingBody).toContain("Loading Helm release details...");
	});

	test("Svelte Helm details use backend reconciliation", () => {
		const surfaces = [
			readFileSync("src/app/svelte/AppSurfaces.svelte", "utf8"),
			readFileSync("src/app/svelte/HelmSurface.svelte", "utf8"),
		].join("\n");

		expect(surfaces).toContain("getHelmReleaseReconciliation");
		expect(surfaces).toContain("queryKeys.helmReleaseReconciliation");
		expect(surfaces).toContain("sortHelmReconciliationResources");
		expect(surfaces).toContain("helmReconciliationStatusLabel");
		expect(surfaces).toContain("Helm reconciliation unavailable");
		expect(surfaces).toContain("No manifest or explicit Helm-labeled live resources were found.");
	});

	test("detail tab uses backend reconciliation instead of fixed-kind listing", () => {
		const source = readFileSync(
			"src/app/svelte/HelmSurface.svelte",
			"utf8",
		);

		expect(source).toContain("helmReconciliationQuery");
		expect(source).toContain("helmReconciliationRows");
		expect(source).toContain("reconciliation");
		expect(source).not.toContain("SUPPORTED_KINDS");
		expect(source).not.toContain("listResources");
		expect(source).not.toContain("HelmReleaseResources");
	});
});

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const detailsSource = readFileSync(
	"src/features/gitops/ArgoApplicationDetails.svelte",
	"utf8",
);
const panelSource = readFileSync(
	"src/features/resource-detail/ResourceDetailPanel.svelte",
	"utf8",
);

describe("Argo Application Briefing contract", () => {
	test("keeps outer resource navigation and one Briefing flow", () => {
		for (const label of [">Details<", ">Events<", ">Argo CD<", ">Actions<", ">YAML<"]) {
			expect(panelSource).toContain(label);
		}
		for (const label of [
			"Change navigator",
			"Delivery",
			"Recent deployment",
			"Application snapshot",
			"Changes",
			"Desired",
			"Live",
		]) expect(detailsSource).toContain(label);
	});

	test("uses production data and operation contracts without prototype simulation", () => {
		for (const contract of [
			"getArgoApplicationInspector",
			"getArgoResourceComparison",
			"preflightArgoOperation",
			"runArgoOperationLifecycle",
			"queryKeys.argoWorkspaceApplication",
		]) expect(detailsSource).toContain(contract);
		for (const forbidden of [
			"createArgoPrototypeFixture",
			"argo-workspace-session",
			"detailsVariation",
			"PrototypeSwitcher",
			"showToast",
			"setTimeout",
			"Local demo",
		]) expect(detailsSource).not.toContain(forbidden);
	});

	test("materializes Application defaults for the primary Sync action", () => {
		expect(detailsSource).toMatch(
			/function syncWithDefaults\(\)[\s\S]*?withArgoSyncSettings\(\s*operation\("sync"\),\s*applicationSyncDefaults/,
		);
	});

	test("keeps accepted refresh retry separate from operation retry", () => {
		expect(detailsSource).toContain("ArgoOperationRefreshError");
		expect(detailsSource).toContain("Retry state refresh");
		expect(detailsSource).toContain("acceptedRefreshPending");
	});

	test("keeps removal explanation, read-only YAML, and accessible progress", () => {
		expect(detailsSource).toContain("Live-only resource");
		expect(detailsSource).toContain("Live YAML stays collapsed until Show diff is selected.");
		expect(detailsSource).toContain('editable={false}');
		expect(detailsSource).toContain('role="status"');
		expect(detailsSource).toContain('aria-live="polite"');
		expect(detailsSource).toContain("motion-reduce:transition-none");
	});

});

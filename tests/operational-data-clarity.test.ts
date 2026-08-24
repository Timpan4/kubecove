import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = (path: string) => readFileSync(path, "utf8");

describe("operational data clarity", () => {
	test("uses one readable timestamp component with exact values available", () => {
		const timestamp = source("src/components/TimestampText.svelte");
		const resources = source("src/features/resources/ResourceBrowser.svelte");
		const namespaces = source("src/features/resources/NamespaceList.svelte");

		expect(timestamp).toContain("formatRelativeTimestamp");
		expect(timestamp).toContain("title={exactTimestamp ?? value}");
		expect(resources).toContain("<TimestampText value={row.createdAt} relative={row.age}");
		expect(namespaces).toContain(
			"<TimestampText value={namespace.createdAt} relative={namespace.age}",
		);
		expect(source("src/features/live-sessions/LiveSessionsView.svelte")).toContain(
			"<TimestampText value={session.startedAt}",
		);
		expect(source("src/features/incidents/IncidentGuide.svelte")).toContain(
			"<TimestampText value={evidence.timestamp}",
		);
		expect(source("src/features/gitops/ArgoApplicationDetails.svelte")).toContain(
			"<TimestampText value={selectedHistory.deployedAt}",
		);
		expect(source("src/features/helm/HelmView.svelte")).toContain(
			"<TimestampText value={details.summary.updatedAt} relative={details.summary.age}",
		);
		expect(source("src/features/rbac/RbacReviewPanel.svelte")).toContain(
			"<TimestampText value={record.reviewedAt}",
		);
	});

	test("explains metric units and unavailable resource state", () => {
		const resources = source("src/features/resources/ResourceBrowser.svelte");
		const details = source("src/features/resource-detail/DetailsTab.svelte");
		const detailPanel = source("src/features/resource-detail/ResourceDetailPanel.svelte");

		expect(resources).toContain("1000m equals one CPU core");
		expect(resources).toContain("binary units such as Ki, Mi, and Gi");
		expect(resources).toContain("Readiness not reported");
		expect(resources).toContain("Restart count not reported");
		expect(details).toContain('Restarts {detailResource.restarts ?? "not reported"}');
		expect(detailPanel).toContain('if (container.ready === undefined) return "Not reported"');
	});

	test("makes truncated resource and GitOps identifiers copyable", () => {
		const resources = source("src/features/resources/ResourceBrowser.svelte");
		const gitOps = source("src/features/gitops/GitOpsView.svelte");

		expect(resources).toContain("copyResourceName");
		expect(resources).toContain(["Copy resource name ", "{row.name}"].join("$"));
		expect(gitOps).toContain("copyRevealedValue");
		expect(gitOps).toContain("Activate to copy");
	});
});

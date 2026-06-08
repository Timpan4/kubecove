import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	helmReconciliationResourceLabel,
	helmReconciliationStatusLabel,
	helmReconciliationStatusTone,
	sortHelmReconciliationResources,
} from "../src/features/helm/helpers";
import type { HelmReconciliationResource } from "../src/lib/types";

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

	test("detail tab uses backend reconciliation instead of fixed-kind listing", () => {
		const source = readFileSync(
			"src/features/helm/HelmDetailPanel.tsx",
			"utf8",
		);

		expect(source).toContain("HelmReconciliationPanel");
		expect(source).toContain('value="reconciliation"');
		expect(source).not.toContain("SUPPORTED_KINDS");
		expect(source).not.toContain("listResources");
		expect(source).not.toContain("HelmReleaseResources");
	});
});

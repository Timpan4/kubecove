import { describe, expect, test } from "bun:test";
import { extractArgoStatusInsights } from "../src/features/argo/status-insights";

describe("argo status insights", () => {
	test("extracts health message, conditions, and unhealthy resources", () => {
		const insights = extractArgoStatusInsights({
			health: {
				status: "Degraded",
				message: "Deployment exceeded its progress deadline",
				lastTransitionTime: "2026-06-05T08:54:18Z",
			},
			conditions: [
				{
					type: "SyncError",
					message: "one or more objects failed to apply",
					lastTransitionTime: "2026-06-05T08:54:18Z",
				},
				{ message: "no type, dropped" },
			],
			resources: [
				{
					kind: "Deployment",
					name: "hubble-relay",
					namespace: "kube-system",
					status: "Synced",
					health: { status: "Degraded", message: "progress deadline exceeded" },
				},
				{
					kind: "Service",
					name: "cilium",
					status: "Synced",
					health: { status: "Healthy" },
				},
			],
		});

		expect(insights.healthMessage).toBe(
			"Deployment exceeded its progress deadline",
		);
		expect(insights.healthTransitionTime).toBe("2026-06-05T08:54:18Z");
		expect(insights.conditions).toHaveLength(1);
		expect(insights.conditions[0].type).toBe("SyncError");
		expect(insights.unhealthyResources).toHaveLength(1);
		expect(insights.unhealthyResources[0]).toMatchObject({
			kind: "Deployment",
			name: "hubble-relay",
			health: "Degraded",
			message: "progress deadline exceeded",
		});
	});

	test("handles empty and malformed status blobs", () => {
		expect(extractArgoStatusInsights(undefined)).toEqual({
			healthMessage: null,
			healthTransitionTime: null,
			conditions: [],
			unhealthyResources: [],
		});
		expect(
			extractArgoStatusInsights({ health: "weird", resources: "nope" })
				.unhealthyResources,
		).toEqual([]);
	});
});

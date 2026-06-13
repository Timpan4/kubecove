import { describe, expect, test } from "bun:test";
import type { ResourceSummary } from "../src/lib/types";
import {
	pageAppGroupCounts,
	pageTypeGroupCounts,
} from "../src/features/resources/grouping";

describe("resource grouping helpers", () => {
	const baseResource: ResourceSummary = {
		cluster: "kind-prod",
		kind: "Pod",
		name: "api-0",
		namespace: "payments",
		age: "3h",
	};

	test("counts page rows by Argo app, owner, and unmanaged fallback", () => {
		const rows: ResourceSummary[] = [
			{ ...baseResource, name: "api", kind: "Pod", argoApp: "payments" },
			{ ...baseResource, name: "svc", kind: "Service", argoApp: "payments" },
			{ ...baseResource, name: "deploy-pod", kind: "Pod", ownerRef: "api" },
			{ ...baseResource, name: "cm", kind: "ConfigMap" },
		];

		expect(pageAppGroupCounts(rows, true)).toEqual(
			new Map([
				["Tracked by Argo CD: payments", 2],
				["Owned by: api", 1],
				["Unmanaged resources", 1],
			]),
		);
	});

	test("counts page rows by app and resource type subgroup", () => {
		const rows: ResourceSummary[] = [
			{ ...baseResource, name: "api", kind: "Pod", argoApp: "payments" },
			{ ...baseResource, name: "svc", kind: "Service", argoApp: "payments" },
			{ ...baseResource, name: "deploy-pod", kind: "Pod", ownerRef: "api" },
			{ ...baseResource, name: "cm", kind: "ConfigMap" },
		];

		expect(pageTypeGroupCounts(rows, true)).toEqual(
			new Map([
				["Tracked by Argo CD: payments::Pods", 1],
				["Tracked by Argo CD: payments::Services", 1],
				["Owned by: api::Pods", 1],
				["Unmanaged resources::ConfigMaps", 1],
			]),
		);
	});

	test("returns empty counts when grouping is disabled", () => {
		expect(pageAppGroupCounts([baseResource], false).size).toBe(0);
		expect(pageTypeGroupCounts([baseResource], false).size).toBe(0);
	});
});

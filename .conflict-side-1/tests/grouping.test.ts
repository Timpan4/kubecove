import { describe, expect, test } from "bun:test";
import type { ResourceSummary } from "../src/lib/types";
import {
	pageGitOpsGroupCounts,
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

	test("counts page rows by GitOps owner and unmanaged fallback", () => {
		const rows: ResourceSummary[] = [
			{ ...baseResource, name: "api", kind: "Pod", argoApp: "payments" },
			{ ...baseResource, name: "svc", kind: "Service", argoApp: "payments" },
			{ ...baseResource, name: "deploy-pod", kind: "Pod", ownerRef: "api" },
			{ ...baseResource, name: "cm", kind: "ConfigMap" },
		];

		expect(pageGitOpsGroupCounts(rows, true)).toEqual(
			new Map([
				["Owned by Argo CD: payments", 2],
				["Unmanaged resources", 2],
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
				["Owned by Argo CD: payments::Pods", 1],
				["Owned by Argo CD: payments::Services", 1],
				["Unmanaged resources::Pods", 1],
				["Unmanaged resources::ConfigMaps", 1],
			]),
		);
	});

	test("returns empty counts when grouping is disabled", () => {
		expect(pageGitOpsGroupCounts([baseResource], false).size).toBe(0);
		expect(pageTypeGroupCounts([baseResource], false).size).toBe(0);
	});
});

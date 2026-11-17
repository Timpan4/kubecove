import { describe, expect, test } from "bun:test";
import {
	pageGitOpsGroupCounts,
	pageTypeGroupCounts,
} from "../src/features/resources/grouping";
import type { ResourceSummary } from "../src/lib/types";

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
				["Owned by Argo CD: payments (partial evidence)", 2],
				["No GitOps ownership evidence", 2],
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
				["group:Owned by Argo CD: payments (partial evidence)::type:Pods", 1],
				["group:Owned by Argo CD: payments (partial evidence)::type:Services", 1],
				["group:No GitOps ownership evidence::type:Pods", 1],
				["group:No GitOps ownership evidence::type:ConfigMaps", 1],
			]),
		);
	});

	test("returns no owner counts when grouping is disabled", () => {
		expect(pageGitOpsGroupCounts([baseResource], false).size).toBe(0);
	});
});

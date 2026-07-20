import { describe, expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { requiredPermissionForFriendlyError } from "../src/lib/friendly-errors";
import { requiredPermissionForResource } from "../src/features/rbac/handoff";

const target = {
	kind: "resource" as const,
	verb: "get",
	apiGroup: "",
	resource: "pods",
	namespace: "prod",
};

describe("RBAC denied-action handoff", () => {
	test("uses only explicit attributes from a forbidden error", () => {
		expect(
			requiredPermissionForFriendlyError(
				{ kind: "forbidden", message: "pods is forbidden" },
				{ requiredPermission: target },
			),
		).toEqual(target);
	});

	test("never guesses attributes for generic or non-forbidden errors", () => {
		expect(
			requiredPermissionForFriendlyError(
				{ kind: "forbidden", message: "forbidden" },
				{},
			),
		).toBeNull();
		expect(
			requiredPermissionForFriendlyError(
				{ kind: "network", message: "timed out" },
				{ requiredPermission: target },
			),
		).toBeNull();
	});

	test("builds a target only from explicit resource API identity", () => {
		expect(
			requiredPermissionForResource(
				{
					cluster: "dev",
					kind: "Widget",
					name: "api",
					namespace: "prod",
					age: "",
					health: "unknown",
					group: "example.io",
					plural: "widgets",
					namespaced: true,
				},
				"get",
			),
		).toEqual({
			kind: "resource",
			verb: "get",
			apiGroup: "example.io",
			resource: "widgets",
			namespace: "prod",
			name: "api",
		});
		expect(
			requiredPermissionForResource(
				{ cluster: "dev", kind: "Pod", name: "api", namespace: "prod", age: "", health: "unknown" },
				"get",
			),
		).toBeNull();
	});

	test("uses one cancellable verifier request slot and cancels it on surface teardown", async () => {
		const [view, surface] = await Promise.all([
			readFile(new URL("../src/features/rbac/RbacView.svelte", import.meta.url), "utf8"),
			readFile(new URL("../src/features/rbac/RbacSurface.svelte", import.meta.url), "utf8"),
		]);
		expect(view).toContain('requestId: "rbac-review"');
		expect(surface).toContain('cancelBackendRequests(client, "rbac-review")');
	});
});

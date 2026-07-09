import { describe, expect, test } from "bun:test";
import {
	applyWorkspaceImport,
	buildWorkspaceImportPreview,
	serializeWorkspaceExport,
} from "../src/lib/workspace-sharing";
import {
	createSavedPortForward,
	createWorkspaceRecord,
	type SavedWorkspace,
} from "../src/lib/workspace-model";

function workspace(name = "Ops"): SavedWorkspace {
	return {
		...createWorkspaceRecord(
			{
				name,
				clusterContext: "kind-dev",
				namespaces: ["payments"],
			},
			"2026-07-01T00:00:00.000Z",
		),
		sharedKey: "ops-workspace",
		portForwards: [
			{
				...createSavedPortForward(
					{
						clusterContext: "kind-dev",
						namespace: "payments",
						serviceName: "payments-api",
						servicePort: 8080,
						localPort: 18080,
						label: "Payments API",
					},
					"2026-07-01T00:00:00.000Z",
				),
				lastStatus: "error",
				lastError: "local port busy",
				lastStartedAt: "2026-07-01T01:00:00.000Z",
			},
		],
	};
}

describe("workspace sharing", () => {
	test("exports Git-friendly workspace JSON without local runtime fields", () => {
		const json = serializeWorkspaceExport([workspace()]);
		const parsed = JSON.parse(json);

		expect(parsed).toMatchObject({
			apiVersion: "kubecove.dev/workspace/v1",
			kind: "Workspace",
			metadata: { name: "ops-workspace" },
			spec: {
				displayName: "Ops",
				portForwards: [
					{
						clusterContext: "kind-dev",
						namespace: "payments",
						serviceName: "payments-api",
						servicePort: 8080,
						localPort: 18080,
						label: "Payments API",
					},
				],
			},
		});
		expect(json).not.toContain("lastError");
		expect(json).not.toContain("lastStatus");
		expect(json).not.toContain("lastStartedAt");
		expect(json).not.toContain("kubeconfig");
		expect(json).not.toContain("createdAt");
		expect(json).not.toContain("updatedAt");
	});

	test("imports single workspace and resets port-forward runtime state", () => {
		const preview = buildWorkspaceImportPreview(serializeWorkspaceExport([workspace()]), []);
		const result = applyWorkspaceImport(
			[],
			preview,
			{ [preview.items[0].id]: "add" },
			"2026-07-02T00:00:00.000Z",
		);

		expect(result.added).toBe(1);
		expect(result.workspaces[0]).toMatchObject({
			sharedKey: "ops-workspace",
			name: "Ops",
			createdAt: "2026-07-02T00:00:00.000Z",
			portForwards: [
				{
					serviceName: "payments-api",
					servicePort: 8080,
					lastStatus: "idle",
				},
			],
		});
		expect(result.workspaces[0].portForwards[0].lastError).toBeUndefined();
	});

	test("lets import choose skip, replace, or copy for collisions", () => {
		const existing = workspace("Ops");
		const imported = {
			...workspace("Ops from Git"),
			scope: { ...existing.scope, namespaces: ["platform"] },
		};
		const preview = buildWorkspaceImportPreview(serializeWorkspaceExport([imported]), [
			existing,
		]);
		const itemId = preview.items[0].id;

		expect(preview.items[0]).toMatchObject({
			existingWorkspaceId: existing.id,
			defaultAction: "skip",
		});
		expect(applyWorkspaceImport([existing], preview, { [itemId]: "skip" }).workspaces).toEqual([
			existing,
		]);

		const replaced = applyWorkspaceImport(
			[existing],
			preview,
			{ [itemId]: "replace" },
			"2026-07-02T00:00:00.000Z",
		).workspaces[0];
		expect(replaced.id).toBe(existing.id);
		expect(replaced.name).toBe("Ops from Git");
		expect(replaced.scope.namespaces).toEqual(["platform"]);

		const copied = applyWorkspaceImport(
			[existing],
			preview,
			{ [itemId]: "add" },
			"2026-07-02T00:00:00.000Z",
		);
		expect(copied.workspaces).toHaveLength(2);
		expect(copied.workspaces[0].name).toBe("Ops from Git");
		expect(copied.workspaces[0].sharedKey).toBe("ops-workspace-2");
	});

	test("accepts WorkspaceList and rejects duplicate Git keys", () => {
		const first = workspace("Ops");
		const second = { ...workspace("DevOps"), sharedKey: "devops-workspace" };
		const listJson = serializeWorkspaceExport([first, second]);

		expect(buildWorkspaceImportPreview(listJson, []).items.map((item) => item.workspace.sharedKey)).toEqual([
			"ops-workspace",
			"devops-workspace",
		]);

		const duplicate = JSON.parse(listJson);
		duplicate.items[1].metadata.name = duplicate.items[0].metadata.name;
		expect(() => buildWorkspaceImportPreview(JSON.stringify(duplicate), [])).toThrow(
			"Duplicate workspace metadata.name",
		);
	});
});

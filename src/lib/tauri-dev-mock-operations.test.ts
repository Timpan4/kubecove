import { expect, test } from "bun:test";
import { createDevMockTauriClient } from "./tauri-dev-mocks";
import type { ClusterOperationResult, ResourceActionPreview, ResourceActionRequest } from "./types";

// The browser mock must hold resource-action reviews to the backend contract:
// one pending, single-use review that only an exact `{ reviewId }` consumes.
const client = createDevMockTauriClient({ latencyMs: 0 });

const deleteConfigMap = (name: string): ResourceActionRequest => ({
	clusterContext: "mock-dev",
	kind: "ConfigMap",
	name,
	namespace: "default",
	apiVersion: "v1",
	action: "delete",
});
const preview = (request: ResourceActionRequest) =>
	client.invoke<ResourceActionPreview>("preview_resource_action", { request });
const execute = (confirmation: unknown) =>
	client.invoke<ClusterOperationResult>("execute_resource_action", { confirmation });

test("a malformed confirmation is refused without consuming the pending review", async () => {
	const { reviewId } = await preview(deleteConfigMap("confirmation-shape"));
	const malformed = [
		undefined,
		null,
		reviewId,
		[reviewId],
		{},
		{ review_id: reviewId },
		{ reviewId: 1 },
		{ reviewId, expectedUid: "mock-uid-1" },
	];
	for (const confirmation of malformed) {
		await expect(execute(confirmation)).rejects.toThrow("Resource action confirmation");
	}
	await execute({ reviewId });
	await expect(execute({ reviewId })).rejects.toThrow("missing, superseded, or already used");
});

test("a later preview whose dry run fails supersedes the pending review", async () => {
	const { reviewId } = await preview(deleteConfigMap("confirmation-superseded"));
	await expect(preview(deleteConfigMap("mock-dry-run-error-superseding"))).rejects.toThrow("dry-run failed");
	await expect(execute({ reviewId })).rejects.toThrow("missing, superseded, or already used");
});

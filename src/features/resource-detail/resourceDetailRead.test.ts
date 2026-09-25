import { describe, expect, test } from "bun:test";
import { createMockTauriClient } from "@/lib/tauri-runtime";
import type { ResourceSummary } from "@/lib/types";
import { readResourceDetails, readResourceYaml, type ResourceReadOptions } from "./resourceDetailReadSpec";

const builtIn: ResourceSummary = {
	cluster: "kind-dev",
	kind: "ConfigMap",
	name: "settings",
	namespace: "default",
	age: "1d",
	health: "unknown",
	apiVersion: "v1",
};
const custom: ResourceSummary = {
	...builtIn,
	kind: "Application",
	name: "demo",
	namespace: "argocd",
	apiVersion: "argoproj.io/v1alpha1",
	group: "argoproj.io",
	version: "v1alpha1",
	plural: "applications",
	namespaced: true,
	dynamic: true,
};
const options: ResourceReadOptions = {
	kubeconfigSourceKey: undefined,
	yamlViewMode: "applyClean",
	yamlEncoding: "yaml",
	cancellable: { requestId: "req-1", cancelScope: "scope-1" },
};

function recordingClient() {
	const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
	const record = (command: string, result: unknown) => (args: Record<string, unknown>) => {
		calls.push({ command, args });
		return result;
	};
	const client = createMockTauriClient({
		get_resource_yaml: record("get_resource_yaml", "kind: ConfigMap"),
		get_resource_details: record("get_resource_details", { yaml: "kind: ConfigMap" }),
		get_dynamic_resource_details: record("get_dynamic_resource_details", { yaml: "kind: Application" }),
	});
	return { client, calls };
}

describe("resource detail reads", () => {
	test("custom resource YAML comes from the discovered-kind details command", async () => {
		const { client, calls } = recordingClient();

		expect(await readResourceYaml(client, custom, options)).toBe("kind: Application");
		expect(calls.map(({ command }) => command)).toEqual(["get_dynamic_resource_details"]);
		expect(calls[0].args).toMatchObject({
			resourceKind: { kind: "Application", plural: "applications", apiVersion: "argoproj.io/v1alpha1" },
			yamlViewMode: "applyClean",
			requestId: "req-1",
			cancelScope: "scope-1",
		});
	});

	test("built-in YAML keeps the YAML command and forwards view options", async () => {
		const { client, calls } = recordingClient();

		expect(await readResourceYaml(client, builtIn, options)).toBe("kind: ConfigMap");
		expect(calls).toEqual([
			{
				command: "get_resource_yaml",
				args: expect.objectContaining({ kind: "ConfigMap", yamlViewMode: "applyClean", yamlEncoding: "yaml", requestId: "req-1" }),
			},
		]);
	});

	test("details follow the same command family", async () => {
		const { client, calls } = recordingClient();

		await readResourceDetails(client, builtIn, options);
		await readResourceDetails(client, custom, options);

		expect(calls.map(({ command }) => command)).toEqual(["get_resource_details", "get_dynamic_resource_details"]);
	});
});

import { describe, expect, test } from "bun:test";
import {
	buildYamlApplyRequest,
	isYamlApplyDisabled,
	yamlAppliedMessage,
	yamlApplyTargetLabel,
} from "../src/features/resource-detail/yamlApplyModel";
import type { ResourceSummary, YamlApplyResult } from "../src/lib/types";

const deployment: ResourceSummary = {
	cluster: "kind-demo",
	kind: "Deployment",
	name: "web",
	namespace: "apps",
	age: "1d",
	apiVersion: "apps/v1",
	group: "apps",
	version: "v1",
	plural: "deployments",
	namespaced: true,
	health: "healthy",
};

describe("svelte YAML apply model", () => {
	test("builds guarded apply requests with visible target scope", () => {
		expect(
			buildYamlApplyRequest({
				resource: deployment,
				kubeconfigSourceKey: "kubeconfigSource=demo",
				yaml: "kind: Deployment\nmetadata:\n  name: web\n",
				yamlEncoding: "yaml",
				forceConflicts: true,
			}),
		).toEqual({
			clusterContext: "kind-demo",
			kubeconfigEnvVar: "kubeconfigSource=demo",
			kind: "Deployment",
			apiVersion: "apps/v1",
			group: "apps",
			version: "v1",
			plural: "deployments",
			namespaced: true,
			name: "web",
			namespace: "apps",
			yaml: "kind: Deployment\nmetadata:\n  name: web\n",
			yamlEncoding: "yaml",
			forceConflicts: true,
		});
		expect(yamlApplyTargetLabel(deployment)).toBe(
			"kind-demo / apps / Deployment / web",
		);
	});

	test("guards v1 Secret apply and describes apply result", () => {
		expect(
			isYamlApplyDisabled({
				...deployment,
				kind: "Secret",
				apiVersion: "v1",
			}),
		).toContain("disabled");
		const result: YamlApplyResult = {
			target: {
				clusterContext: "kind-demo",
				kind: "Deployment",
				name: "web",
				namespace: "apps",
			},
			appliedYaml: "kind: Deployment\n",
		};
		expect(yamlAppliedMessage(result, true)).toBe(
			"Deployment/web applied with force-conflicts.",
		);
	});
});

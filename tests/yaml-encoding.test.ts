import { describe, expect, test } from "bun:test";
import { formatYamlDocument } from "../src/lib/yamlFormat";
import { queryKeys } from "../src/lib/queryKeys";
import { useSettingsState } from "../src/lib/settings";

describe("YAML encoding", () => {
	test("formats KYAML with bare keys and quoted string values", () => {
		const formatted = formatYamlDocument(
			[
				"apiVersion: v1",
				"kind: ConfigMap",
				"metadata:",
				"  name: app-config",
				"  labels:",
				"    app.kubernetes.io/name: api",
				"data:",
				"  country: NO",
				"  enabled: 'true'",
				"  answer: 42",
				"  flag: true",
			].join("\n"),
			"kyaml",
		);

		expect(formatted).toContain('apiVersion: "v1"');
		expect(formatted).toContain('app.kubernetes.io/name: "api"');
		expect(formatted).toContain('country: "NO"');
		expect(formatted).toContain('enabled: "true"');
		expect(formatted).toContain("answer: 42");
		expect(formatted).toContain("flag: true");
		expect(formatted).not.toContain('"apiVersion":');
	});

	test("formats flow KYAML input back to block YAML when YAML is active", () => {
		const formatted = formatYamlDocument(
			'{ apiVersion: "v1", kind: "Pod", metadata: { name: "redis", namespace: "argocd" } }',
			"yaml",
		);

		expect(formatted).toContain("apiVersion: v1");
		expect(formatted).toContain("kind: Pod");
		expect(formatted).toContain("metadata:\n  name: redis");
		expect(formatted).not.toContain('{ apiVersion: "v1"');
	});

	test("helm detail keys include cleanup shape and encoding", () => {
		const yamlKey = queryKeys.helmReleaseDetails(
			"kind-dev",
			"default",
			"Secret",
			"sh.helm.release.v1.api.v1",
			"KUBECONFIG",
			"kubectl",
			"yaml",
		);
		const kyamlKey = queryKeys.helmReleaseDetails(
			"kind-dev",
			"default",
			"Secret",
			"sh.helm.release.v1.api.v1",
			"KUBECONFIG",
			"applyClean",
			"kyaml",
		);

		expect(yamlKey).toContain("kubectl");
		expect(yamlKey).toContain("yaml");
		expect(kyamlKey).toContain("applyClean");
		expect(kyamlKey).toContain("kyaml");
		expect(yamlKey).not.toEqual(kyamlKey);
	});

	test("Argo detail keys include cleanup shape and encoding", () => {
		const yamlKey = queryKeys.argoAppDetails(
			"kind-dev",
			"guestbook",
			"argocd",
			"KUBECONFIG",
			"kubectl",
			"yaml",
		);
		const kyamlKey = queryKeys.argoAppDetails(
			"kind-dev",
			"guestbook",
			"argocd",
			"KUBECONFIG",
			"applyClean",
			"kyaml",
		);

		expect(yamlKey).toContain("argo-app-details");
		expect(yamlKey).toContain("kubectl");
		expect(yamlKey).toContain("yaml");
		expect(kyamlKey).toContain("applyClean");
		expect(kyamlKey).toContain("kyaml");
		expect(yamlKey).not.toEqual(kyamlKey);
	});

	test("resource detail keys are shared across runtimes", () => {
		const resource = {
			apiVersion: "v1",
			cluster: "kind-dev",
			kind: "Pod",
			name: "argocd-server-abc",
			namespace: "argocd",
			age: "7d",
			health: "healthy",
		} as const;
		const kubectl = { yamlViewMode: "kubectl", yamlEncoding: "yaml" } as const;
		const applyClean = { yamlViewMode: "applyClean", yamlEncoding: "kyaml" } as const;
		const detailKey = queryKeys.resourceDetails(resource, "KUBECONFIG", kubectl);
		const yamlKey = queryKeys.resourceYaml(resource, "KUBECONFIG", applyClean);

		expect(queryKeys.resourceDetails({ ...resource }, "KUBECONFIG", kubectl)).toEqual(detailKey);
		expect(queryKeys.resourceDetails(resource, "KUBECONFIG", applyClean)).not.toEqual(detailKey);
		expect(queryKeys.resourceDetails(resource, "OTHER", kubectl)).not.toEqual(detailKey);
		expect(yamlKey).toContain("applyClean");
		expect(yamlKey).toContain("kyaml");
		expect(queryKeys.resourceEvents({ ...resource }, "KUBECONFIG")).toEqual(
			queryKeys.resourceEvents(resource, "KUBECONFIG"),
		);
	});

	test("settings disallow YAML force-conflicts by default and can enable them", () => {
		expect(useSettingsState.getState().allowYamlForceConflicts).toBe(false);

		useSettingsState.getState().setAllowYamlForceConflicts(true);

		expect(useSettingsState.getState().allowYamlForceConflicts).toBe(true);
	});
});

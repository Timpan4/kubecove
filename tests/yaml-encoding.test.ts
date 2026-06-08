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

	test("settings persist separate cleanup and encoding defaults", () => {
		useSettingsState.setState({
			yamlViewModeDefault: "kubectl",
			yamlEncodingDefault: "yaml",
		});

		useSettingsState.getState().setYamlViewModeDefault("applyClean");
		useSettingsState.getState().setYamlEncodingDefault("kyaml");

		expect(useSettingsState.getState().yamlViewModeDefault).toBe("applyClean");
		expect(useSettingsState.getState().yamlEncodingDefault).toBe("kyaml");
	});
});

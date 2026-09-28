import { buildYamlApplyRequest, resolveYamlForceConflicts, sameYamlApplyRequest } from "./yamlApplyModel";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
};

describe("resolveYamlForceConflicts", () => {
	test("ignores click-event-shaped overrides", () => {
		const clickLike = { currentTarget: "dry-run-button" };

		expect(resolveYamlForceConflicts(clickLike, false)).toBe(false);
		expect(resolveYamlForceConflicts(clickLike, true)).toBe(true);
		expect(resolveYamlForceConflicts(true, false)).toBe(true);
	});
});

describe("sameYamlApplyRequest", () => {
	test("detects a kubeconfig source change between preview and apply", () => {
		const reviewed = buildYamlApplyRequest({
			resource: {
				cluster: "kind-dev",
				kind: "ConfigMap",
				name: "settings",
				namespace: "default",
				age: "1d",
				health: "unknown",
				apiVersion: "v1",
			},
			kubeconfigSourceKey: "source-a",
			yaml: "kind: ConfigMap",
			forceConflicts: false,
		});
		const current = { ...reviewed, kubeconfigEnvVar: "source-b" };

		expect(sameYamlApplyRequest(reviewed, current)).toBe(false);
		expect(sameYamlApplyRequest(reviewed, reviewed)).toBe(true);
	});
});

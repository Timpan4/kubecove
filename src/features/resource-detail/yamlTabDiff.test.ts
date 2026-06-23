import { buildYamlDryRunDiff } from "./yamlTabDiff";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toContain(expected: unknown): void;
};

describe("buildYamlDryRunDiff", () => {
	const currentYaml = [
		"apiVersion: apps/v1",
		"kind: Deployment",
		"metadata:",
		"  name: todo-web",
		"spec:",
		"  replicas: 1",
		"  revisionHistoryLimit: 10",
		"",
	].join("\n");
	const dryRunYaml = currentYaml.replace("  replicas: 1", "  replicas: 2");

	test("renders clean compact diff without git headers", () => {
		const texts = buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "clean",
			full: false,
			forceConflicts: false,
		}).map((line) => line.text);

		expect(texts.includes("--- current")).toBe(false);
		expect(texts.some((text) => text.startsWith("@@ "))).toBe(false);
		expect(texts).toContain("-  replicas: 1");
		expect(texts).toContain("+  replicas: 2");
		expect(texts).toContain("  apply gate: dry-run succeeded");
		expect(texts).toContain("  force-conflicts: false");
	});

	test("keeps git-style headers when requested", () => {
		const texts = buildYamlDryRunDiff({
			currentYaml,
			dryRunYaml,
			style: "git",
			full: false,
			forceConflicts: false,
		}).map((line) => line.text);

		expect(texts).toContain("--- current");
		expect(texts).toContain("+++ dry-run");
		expect(texts.some((text) => text.startsWith("@@ "))).toBe(true);
	});

	test("expands context in full mode", () => {
		const current = Array.from(
			{ length: 12 },
			(_, index) => `line${index}: ${index === 6 ? "old" : "same"}`,
		).join("\n");
		const dryRun = current.replace("line6: old", "line6: new");
		const compactTexts = buildYamlDryRunDiff({
			currentYaml: current,
			dryRunYaml: dryRun,
			style: "clean",
			full: false,
			forceConflicts: true,
		}).map((line) => line.text);
		const fullTexts = buildYamlDryRunDiff({
			currentYaml: current,
			dryRunYaml: dryRun,
			style: "clean",
			full: true,
			forceConflicts: true,
		}).map((line) => line.text);

		expect(compactTexts.some((text) => text.includes("line0: same"))).toBe(false);
		expect(fullTexts.some((text) => text.includes("line0: same"))).toBe(true);
		expect(fullTexts).toContain("  force-conflicts: true");
	});
});

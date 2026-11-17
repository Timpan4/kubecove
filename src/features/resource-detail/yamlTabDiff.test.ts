import { buildArgoResourceDiff, buildCompactUnifiedDiff, buildYamlDryRunDiff } from "./yamlTabDiff";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toContain<Expected>(expected: Expected): void;
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

	test("labels Argo target and live YAML", () => {
		const texts = buildArgoResourceDiff(currentYaml, dryRunYaml).map((line) => line.text);
		expect(texts).toContain("--- target");
		expect(texts).toContain("+++ live");
		expect(texts).toContain("-  replicas: 1");
		expect(texts).toContain("+  replicas: 2");
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

// Hunk ranges can break in these ways: an empty old side or an empty new side
// prints jsdiff's start (the line after the range) instead of the line before
// it; both sides empty must still produce no hunk; insertions at the start or
// end of a non-empty file must keep their 1-based starts. With three context
// lines an empty side only occurs for an empty file, so its start is 0.
describe("unified hunk ranges", () => {
	const hunks = (before: string, after: string) =>
		buildCompactUnifiedDiff(before, after)
			.filter((line) => line.type === "hunk")
			.map((line) => line.text);

	test("an empty side names the line before it", () => {
		expect(hunks("", "a\nb\n").join("|")).toBe("@@ -0,0 +1,2 @@");
		expect(hunks("a\nb\n", "").join("|")).toBe("@@ -1,2 +0,0 @@");
	});

	test("edits at either end keep 1-based starts", () => {
		expect(hunks("b\nc\n", "a\nb\nc\n").join("|")).toBe("@@ -1,2 +1,3 @@");
		expect(hunks("a\nb\n", "a\nb\nc\n").join("|")).toBe("@@ -1,2 +1,3 @@");
	});

	test("two empty sides report no changes", () => {
		expect(hunks("", "").length).toBe(0);
		expect(buildCompactUnifiedDiff("", "").map((line) => line.text)).toContain("No server-side dry-run changes.");
	});
});

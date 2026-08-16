import { RuleTester } from "oxlint/plugins-dev";

import { noKnownValueWideningRule } from "../tools/oxlint/anti-slop/rules/no-known-value-widening.ts";
import { noModuleMockingRule } from "../tools/oxlint/anti-slop/rules/no-module-mocking.ts";
import { noObjectParametersRule } from "../tools/oxlint/anti-slop/rules/no-object-parameters.ts";
import { noUnknownParametersRule } from "../tools/oxlint/anti-slop/rules/no-unknown-parameters.ts";
import { noUnknownTypeAliasesRule } from "../tools/oxlint/anti-slop/rules/no-unknown-type-aliases.ts";
import { noUnsafeDictionaryTypeRule } from "../tools/oxlint/anti-slop/rules/no-unsafe-dictionary-type.ts";
import { noWidenThenAssertRule } from "../tools/oxlint/anti-slop/rules/no-widen-then-assert.ts";

function runRegressionCases(): void {
	const tester = () =>
		new RuleTester({ languageOptions: { parserOptions: { lang: "ts" } } });

	tester().run("anti-slop/no-known-value-widening", noKnownValueWideningRule, {
		valid: [],
		invalid: [
			{
				code: "const value: object = { id: 'one' } as object;",
				errors: 1,
			},
		],
	});

	tester().run("anti-slop/no-unknown-type-aliases", noUnknownTypeAliasesRule, {
		valid: [],
		invalid: [
			{
				code: "type Hidden = unknown; type Generic<Hidden> = Hidden;",
				errors: 1,
			},
		],
	});

	tester().run("anti-slop/no-widen-then-assert", noWidenThenAssertRule, {
		valid: [
			"type Record<K, V> = { key: K; value: V }; const source = { id: 'one' }; const widened: Record<string, unknown> = source; const parsed = widened as { id: string };",
			"type Readonly<T> = { value: T }; const source = { id: 'one' }; const widened: Readonly<Record<string, unknown>> = source; const parsed = widened as { id: string };",
			"type PropertyKey = 'id'; const source = { id: 'one' }; const widened: Record<PropertyKey, unknown> = source; const parsed = widened as { id: string };",
		],
		invalid: [
			{
				code: "const source = { id: 'one' }; const widened: Record<string, unknown> = source; const parsed = widened as { id: string };",
				errors: 1,
			},
		],
	});

	tester().run("anti-slop/no-unknown-parameters", noUnknownParametersRule, {
		valid: [],
		invalid: [{ code: "function parse(value: unknown) {}", errors: 1 }],
	});

	tester().run("anti-slop/no-object-parameters", noObjectParametersRule, {
		valid: [],
		invalid: [{ code: "function parse(value: object) {}", errors: 1 }],
	});

	tester().run("anti-slop/no-module-mocking", noModuleMockingRule, {
		valid: [],
		invalid: [
			{
				code: "import { vi } from 'vitest'; vi.mock('./dependency');",
				errors: 1,
			},
		],
	});

	tester().run("anti-slop/no-unsafe-dictionary-type", noUnsafeDictionaryTypeRule, {
		valid: [],
		invalid: [{ code: "type Unsafe = Record<string, unknown & {}>;", errors: 1 }],
	});
}

if (process.versions.bun === undefined) {
	runRegressionCases();
} else {
	const { expect, test } = await import("bun:test");
	test("anti-slop plugin review regressions", () => {
		const result = Bun.spawnSync([
			"node",
			"--experimental-strip-types",
			import.meta.path,
		]);
		expect(result.stderr.toString()).toBe("");
		expect(result.exitCode).toBe(0);
	});
}

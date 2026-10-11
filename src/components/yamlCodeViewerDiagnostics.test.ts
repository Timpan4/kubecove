import { expect, test } from "bun:test";
import type { Diagnostic } from "@codemirror/lint";
import { RangeSetBuilder, Text } from "@codemirror/state";
import { Decoration } from "@codemirror/view";
import { forEachYamlErrorLensDiagnostic } from "./yamlCodeViewerDiagnostics";

test("error lens emits sorted ranges and preserves the first visible diagnostic per line", () => {
	const doc = Text.of(["first: true", "second: false", "third: null"]);
	const diagnostics: Diagnostic[] = [
		{
			from: doc.line(3).from,
			to: doc.line(3).to,
			severity: "warning",
			message: "third",
		},
		{
			from: doc.line(1).from,
			to: doc.line(1).to,
			severity: "info",
			message: "info",
		},
		{
			from: doc.line(2).from + 1,
			to: doc.line(2).to,
			severity: "warning",
			message: "first on second",
		},
		{
			from: doc.line(1).from,
			to: doc.line(1).to,
			severity: "error",
			message: "first",
		},
		{
			from: doc.line(2).from,
			to: doc.line(2).to,
			severity: "error",
			message: "later on second",
		},
	];
	const original = [...diagnostics];
	const selected: string[] = [];
	const builder = new RangeSetBuilder<Decoration>();
	forEachYamlErrorLensDiagnostic(diagnostics, doc, (to, diagnostic) => {
		selected.push(diagnostic.message);
		builder.add(to, to, Decoration.mark({ class: "diagnostic" }));
	});
	expect(builder.finish().size).toBe(3);
	expect(selected).toEqual(["first", "first on second", "third"]);
	expect(diagnostics).toEqual(original);
});

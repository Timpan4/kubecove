import { expect, test } from "bun:test";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import type { DecorationSet } from "@codemirror/view";
import {
	buildYamlValueDecorations,
	updateYamlValueDecorations,
} from "./yamlCodeViewerExtensions";

const yaml = [
	"apiVersion: v1",
	"kind: ConfigMap",
	"metadata:",
	"  name: demo",
	"data:",
	"  enabled: true",
	"  count: 3",
	'  note: "quoted"',
].join("\n");

function decorationRanges(set: DecorationSet) {
	const result: Array<[number, number, string | undefined]> = [];
	for (let cursor = set.iter(); cursor.value; cursor.next()) {
		result.push([cursor.from, cursor.to, cursor.value.spec.class]);
	}
	return result;
}

function expectIncrementalMatchesRebuild(
	state: EditorState,
	spec: TransactionSpec,
) {
	const transaction = state.update(spec);
	const incremental = updateYamlValueDecorations(
		buildYamlValueDecorations(state),
		transaction,
	);
	const rebuilt = buildYamlValueDecorations(transaction.state);
	expect(decorationRanges(rebuilt).length).toBeGreaterThan(0);
	expect(decorationRanges(incremental)).toEqual(decorationRanges(rebuilt));
}

test("incremental YAML decorations match a full rebuild after deleting a decorated line", () => {
	const state = EditorState.create({ doc: yaml });
	const line = state.doc.line(7);
	expectIncrementalMatchesRebuild(state, {
		changes: { from: line.from, to: state.doc.line(8).from },
	});
});

test("incremental YAML decorations match a full rebuild when inserting at a decoration endpoint", () => {
	const state = EditorState.create({ doc: yaml });
	const trueStart = yaml.indexOf("true");
	const countEnd = yaml.indexOf("3") + 1;
	expectIncrementalMatchesRebuild(state, {
		changes: { from: trueStart, insert: "1" },
	});
	expectIncrementalMatchesRebuild(state, {
		changes: { from: countEnd, insert: "\n  extra: 4" },
	});
});

test("incremental YAML decorations match a full rebuild when the whole document is replaced", () => {
	const state = EditorState.create({ doc: yaml });
	expectIncrementalMatchesRebuild(state, {
		changes: {
			from: 0,
			to: state.doc.length,
			insert: "kind: Secret\ntype: Opaque\ndata:\n  token: 42\n  ok: false\n",
		},
	});
});

test("incremental YAML decorations match a full rebuild after scattered edits", () => {
	const state = EditorState.create({ doc: `${yaml}\n---\n`.repeat(40) });
	const joinedLine = state.doc.line(32);
	expectIncrementalMatchesRebuild(state, {
		changes: [
			{ from: state.doc.line(6).to, insert: " # enabled" },
			{ from: joinedLine.to, to: joinedLine.to + 1 },
			{ from: state.doc.line(133).from + 3, insert: "\n  other" },
			{ from: state.doc.line(358).from, to: state.doc.line(358).to, insert: "  count: false" },
		],
	});
});

test("YAML decorations match a full rebuild when many scattered line groups change", () => {
	const copies = 80;
	const document = `${yaml}\n---\n`;
	const linesPerCopy = document.split("\n").length - 1;
	const state = EditorState.create({ doc: document.repeat(copies) });
	const changes = Array.from({ length: copies }, (_, index) => {
		const line = state.doc.line(index * linesPerCopy + 6);
		return { from: line.from, to: line.to, insert: "  enabled: false # edited" };
	});
	expectIncrementalMatchesRebuild(state, { changes });
});

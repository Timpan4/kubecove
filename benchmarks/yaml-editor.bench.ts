import assert from "node:assert/strict";
import type { Diagnostic } from "@codemirror/lint";
import { EditorState, RangeSetBuilder, Text } from "@codemirror/state";
import { Decoration, WidgetType } from "@codemirror/view";
import { bench, describe } from "vitest";
import {
	forEachYamlErrorLensDiagnostic,
	yamlDiagnostics,
} from "@/components/yamlCodeViewerDiagnostics";
import { buildYamlValueDecorations } from "@/components/yamlCodeViewerExtensions";
import { mapKubernetesYamlDiagnostics } from "@/features/resource-detail/yamlLintDiagnostics";
import type { KubernetesYamlLintDiagnostic } from "@/lib/types";
import { buildYamlManifest } from "./yaml-benchmark-fixtures";

class HeadlessErrorLensWidget extends WidgetType {
	constructor(private readonly diagnostic: Diagnostic) {
		super();
	}

	toDOM() {
		const element = document.createElement("span");
		element.textContent = this.diagnostic.message;
		return element;
	}
}

const yaml = buildYamlManifest(40);
const changedYaml = yaml.replace("replicas: 4", "replicas: 8");
const invalidYaml = `${yaml}\nunfinished: [\n`;
const returnedDiagnostics: KubernetesYamlLintDiagnostic[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		severity: index % 4 === 0 ? "warning" : "error",
		source: index % 2 === 0 ? "kubeconform" : "schema",
		message: `field ${index} failed validation`,
		fieldPath:
			index % 3 === 0
				? "metadata.name"
				: index % 3 === 1
					? "spec.template.spec.containers[39].resources.limits.memory"
					: `spec.missing${index}`,
	}),
);
const mappedDiagnostics = mapKubernetesYamlDiagnostics(
	yaml,
	returnedDiagnostics,
);
const lensDiagnostics: Diagnostic[] = [
	...mappedDiagnostics,
	...mappedDiagnostics
		.slice(0, 100)
		.map((diagnostic) => ({ ...diagnostic, severity: "info" as const })),
];
const editorDocument = Text.of(yaml.split("\n"));
const initialEditorState = EditorState.create({ doc: editorDocument });
const replicaOffset = yaml.indexOf("replicas: 4") + "replicas: ".length;
const changedEditorState = initialEditorState.update({
	changes: { from: replicaOffset, to: replicaOffset + 1, insert: "8" },
}).state;
const initialDecorations = buildYamlValueDecorations(initialEditorState);
const changedDecorations = buildYamlValueDecorations(changedEditorState);
let mappedLensCount = 0;
forEachYamlErrorLensDiagnostic(lensDiagnostics, editorDocument, () => {
	mappedLensCount += 1;
});

assert(yamlDiagnostics(invalidYaml).length > 0);
assert.equal(yamlDiagnostics(yaml).length, 0);
assert.equal(changedEditorState.doc.toString(), changedYaml);
assert.equal(mappedDiagnostics.length, returnedDiagnostics.length);
assert(initialDecorations.size > 0 && changedDecorations.size > 0);
assert(mappedLensCount > 0 && mappedLensCount < lensDiagnostics.length);
assert.equal(buildErrorLensDecorations().size, mappedLensCount);

function buildErrorLensDecorations() {
	const builder = new RangeSetBuilder<Decoration>();
	forEachYamlErrorLensDiagnostic(
		lensDiagnostics,
		editorDocument,
		(to, diagnostic) => {
			builder.add(
				to,
				to,
				Decoration.widget({
					widget: new HeadlessErrorLensWidget(diagnostic),
					side: 1,
				}),
			);
		},
	);
	return builder.finish();
}

describe("F21 lint result application with folded F17 and F23, plus F22 editor updates", () => {
	bench("F21 full valid document local YAML lint", () => {
		yamlDiagnostics(yaml);
	});

	bench("F21 full unfinished document local YAML lint", () => {
		yamlDiagnostics(invalidYaml);
	});

	bench(
		"F21 prepared returned lint result range mapping with folded F17",
		() => {
			mapKubernetesYamlDiagnostics(yaml, returnedDiagnostics);
		},
	);

	bench("F23 grouped error lens mapping over prepared diagnostics", () => {
		buildErrorLensDecorations();
	});

	bench("F22 full decoration build for initial YAML document", () => {
		buildYamlValueDecorations(initialEditorState);
	});

	bench("F22 full decoration rebuild after a document transaction", () => {
		buildYamlValueDecorations(changedEditorState);
	});

	bench("F22 edited document serialization for the update listener", () => {
		changedEditorState.doc.toString();
	});
});

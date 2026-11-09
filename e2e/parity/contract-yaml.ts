import type { Step, YamlEditorDiagnostic } from "./contract-steps";
import { count, object, oneOf, string } from "./contract-values";

/** Parses YAML editor diagnostic steps; returns undefined for any other action. */
export function yamlEditorStep(item: Record<string, unknown>): Step | undefined {
	switch (item.action) {
		case "expectYamlEditorDiagnostics":
			if (!Array.isArray(item.diagnostics)) throw new Error("Expected a list of editor diagnostics");
			return { action: item.action, diagnostics: item.diagnostics.map(yamlEditorDiagnostic) };
		default: return undefined;
	}
}

function yamlEditorDiagnostic(value: unknown): YamlEditorDiagnostic {
	const item = object(value);
	return {
		from: position(item.from),
		to: position(item.to),
		severity: oneOf(["error", "warning", "info"], item.severity, "a diagnostic severity"),
		source: string(item.source),
		...(item.message === undefined ? {} : { message: string(item.message) }),
	};
}

/** A 1-based `[line, column]` editor position. */
function position(value: unknown): [number, number] {
	if (!Array.isArray(value) || value.length !== 2) throw new Error("Expected [line, column]");
	const [line, column] = value.map(count);
	if (!line || !column) throw new Error("Lines and columns start at 1");
	return [line, column];
}

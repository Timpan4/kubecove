import type { Diagnostic } from "@codemirror/lint";
import type { KubernetesYamlLintDiagnostic } from "@/lib/types";
import { findYamlFieldRange } from "./yamlTabDiff";

export function mapKubernetesYamlDiagnostics(
	value: string,
	diagnostics: KubernetesYamlLintDiagnostic[],
): Diagnostic[] {
	return diagnostics.map((diagnostic) => {
		const range = findYamlFieldRange(value, diagnostic.fieldPath);
		return {
			from: range.from,
			to: range.to,
			severity: diagnostic.severity,
			source: diagnostic.source,
			message: diagnostic.message,
		};
	});
}

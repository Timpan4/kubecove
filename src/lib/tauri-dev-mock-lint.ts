import { parseDocument } from "yaml";
import type { JsonObject, JsonValue, KubernetesYamlLintDiagnostic, KubernetesYamlLintResult, YamlApplyRequest } from "./types";

/**
 * The browser mock's `lint_kubernetes_yaml`. It reports the YAML parse error
 * the backend reports, and the apiVersion and kind checks against the selected
 * resource. The backend's metadata, schema and kubeconform checks have no mock
 * counterpart, so they never appear here.
 */
export function lintMockYaml(request: Partial<YamlApplyRequest> | undefined): KubernetesYamlLintResult {
	const yaml = request?.yaml;
	if (!request || yaml === undefined || yaml.trim().length === 0) return { diagnostics: [], notes: [] };
	const document = parseDocument(yaml, { prettyErrors: false, strict: true, uniqueKeys: true });
	const parseError = document.errors[0];
	if (parseError) return { diagnostics: [{ severity: "error", source: "YAML", message: parseError.message }], notes: [] };
	const manifest: JsonValue = document.toJS();
	if (!isRecord(manifest)) return { diagnostics: [], notes: [] };
	return { diagnostics: [...apiVersionDiagnostics(request, manifest), ...kindDiagnostics(request, manifest)], notes: [] };
}

function apiVersionDiagnostics(request: Partial<YamlApplyRequest>, manifest: JsonObject): KubernetesYamlLintDiagnostic[] {
	const actual = manifest.apiVersion;
	if (actual === undefined) return [kubernetesError("apiVersion is required.", "apiVersion")];
	if (request.apiVersion === undefined || actual === request.apiVersion) return [];
	return [kubernetesError(`apiVersion must match selected resource: expected ${request.apiVersion}, got ${String(actual)}.`, "apiVersion")];
}

function kindDiagnostics(request: Partial<YamlApplyRequest>, manifest: JsonObject): KubernetesYamlLintDiagnostic[] {
	const actual = manifest.kind;
	if (actual === undefined) return [kubernetesError("kind is required.", "kind")];
	if (actual === request.kind) return [];
	return [kubernetesError(`kind must match selected resource: expected ${request.kind}, got ${String(actual)}.`, "kind")];
}

function kubernetesError(message: string, fieldPath: string): KubernetesYamlLintDiagnostic {
	return { severity: "error", source: "Kubernetes", message, fieldPath };
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

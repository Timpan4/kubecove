export type YamlViewMode = "kubectl" | "applyClean";
export type YamlEncoding = "yaml" | "kyaml";

export interface YamlApplyRequest {
	clusterContext: string;
	kubeconfigEnvVar?: string;
	kind: string;
	apiVersion?: string;
	group?: string;
	version?: string;
	plural?: string;
	namespaced?: boolean;
	name: string;
	namespace?: string | null;
	yaml: string;
	yamlEncoding?: YamlEncoding;
	forceConflicts?: boolean;
	expectedUid?: string;
	expectedResourceVersion?: string;
}

export interface YamlApplyTarget {
	clusterContext: string;
	kind: string;
	apiVersion?: string;
	name: string;
	namespace?: string | null;
}

export interface YamlApplyPreview {
	target: YamlApplyTarget;
	currentYaml: string;
	dryRunYaml: string;
	uid: string;
	resourceVersion: string;
}

export interface YamlApplyResult {
	target: YamlApplyTarget;
	appliedYaml: string;
}

export type KubernetesYamlLintSeverity = "error" | "warning" | "info";

export interface KubernetesYamlLintDiagnostic {
	severity: KubernetesYamlLintSeverity;
	source: string;
	message: string;
	fieldPath?: string;
}

export interface KubernetesYamlLintStatusNote {
	severity: KubernetesYamlLintSeverity;
	source: string;
	message: string;
}

export interface KubernetesYamlLintResult {
	diagnostics: KubernetesYamlLintDiagnostic[];
	notes: KubernetesYamlLintStatusNote[];
}

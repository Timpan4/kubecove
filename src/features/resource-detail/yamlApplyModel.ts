import type { ResourceSummary, YamlApplyRequest, YamlApplyResult, YamlEncoding } from "@/lib/types";

export function isYamlApplyDisabled(resource: ResourceSummary): string | null {
	return resource.kind === "Secret" && (resource.apiVersion ?? "v1") === "v1"
		? "Secret YAML apply is disabled to avoid exposing sensitive data."
		: null;
}

export function buildYamlApplyRequest({
	resource,
	kubeconfigSourceKey,
	yaml,
	yamlEncoding,
	forceConflicts,
}: {
	resource: ResourceSummary;
	kubeconfigSourceKey?: string;
	yaml: string;
	yamlEncoding?: YamlEncoding;
	forceConflicts: boolean;
}): YamlApplyRequest {
	return {
		clusterContext: resource.cluster,
		kubeconfigEnvVar: kubeconfigSourceKey,
		kind: resource.kind,
		apiVersion: resource.apiVersion,
		group: resource.group,
		version: resource.version,
		plural: resource.plural,
		namespaced: resource.namespaced,
		name: resource.name,
		namespace: resource.namespace,
		yaml,
		yamlEncoding,
		forceConflicts,
	};
}

export function resolveYamlForceConflicts(
	override: unknown,
	fallback: boolean,
): boolean {
	return typeof override === "boolean" ? override : fallback;
}

export function yamlApplyTargetLabel(resource: ResourceSummary): string {
	return [
		resource.cluster,
		resource.namespace ?? "cluster",
		resource.kind,
		resource.name,
	].join(" / ");
}

export function yamlAppliedMessage(result: YamlApplyResult, forceConflicts: boolean): string {
	return `${result.target.kind}/${result.target.name} applied${forceConflicts ? " with force-conflicts" : ""}.`;
}

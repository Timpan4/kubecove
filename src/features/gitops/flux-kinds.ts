import type { FluxResourceKind } from "@/lib/types";

export const FLUX_KIND_LABELS: Record<string, string> = {
	"Flux GitRepositories": "GitRepository",
	"Flux OCIRepositories": "OCIRepository",
	"Flux HelmRepositories": "HelmRepository",
	"Flux HelmCharts": "HelmChart",
	"Flux Buckets": "Bucket",
	"Flux Kustomizations": "Kustomization",
	"Flux HelmReleases": "HelmRelease",
	"Flux Providers": "Provider",
	"Flux Alerts": "Alert",
	"Flux Receivers": "Receiver",
	"Flux ImageRepositories": "ImageRepository",
	"Flux ImagePolicies": "ImagePolicy",
	"Flux ImageUpdateAutomations": "ImageUpdateAutomation",
};

export function fluxKindFromLabel(
	label: string | null,
	kinds: FluxResourceKind[],
): FluxResourceKind | null {
	const kind = label ? FLUX_KIND_LABELS[label] : null;
	if (!kind) return null;
	return kinds.find((candidate) => candidate.kind === kind) ?? null;
}

export function isFluxKindLabel(label: string | null): boolean {
	return Boolean(label && FLUX_KIND_LABELS[label]);
}

export function fluxStatusTone(status?: string | null) {
	if (status === "True") return "success";
	if (status === "False") return "error";
	return "warning";
}

import type { FluxResourceKind } from "@/lib/types";

export const ARGO_PROVIDER_GROUP_ID = "gitops:argo";
export const FLUX_PROVIDER_GROUP_ID = "gitops:flux";

export type ArgoKindKey = "applications" | "applicationSets" | "appProjects";

export interface ArgoNavKind {
	key: ArgoKindKey;
	label: string;
	legacyLabels: string[];
}

export const ARGO_NAV_KINDS: ArgoNavKind[] = [
	{
		key: "applications",
		label: "Argo CD Applications",
		legacyLabels: ["Applications"],
	},
	{
		key: "applicationSets",
		label: "Argo CD ApplicationSets",
		legacyLabels: ["ApplicationSets"],
	},
	{
		key: "appProjects",
		label: "Argo CD AppProjects",
		legacyLabels: ["AppProjects"],
	},
];

export function normalizeArgoKindLabel(label: string | null): ArgoKindKey | null {
	if (!label) return null;
	return (
		ARGO_NAV_KINDS.find(
			(kind) => kind.label === label || kind.legacyLabels.includes(label),
		)?.key ?? null
	);
}

export interface FluxNavKind {
	kind: string;
	label: string;
	legacyLabels: string[];
	family: FluxFamilyKey;
}

export type FluxFamilyKey =
	| "sources"
	| "workloads"
	| "notifications"
	| "imageAutomation";

export interface FluxFamily {
	key: FluxFamilyKey;
	label: string;
	groupId: string;
	kinds: FluxNavKind[];
}

const FLUX_NAV_KINDS: FluxNavKind[] = [
	{
		kind: "GitRepository",
		label: "Flux Git Repositories",
		legacyLabels: ["Flux GitRepositories"],
		family: "sources",
	},
	{
		kind: "OCIRepository",
		label: "Flux OCI Repositories",
		legacyLabels: ["Flux OCIRepositories"],
		family: "sources",
	},
	{
		kind: "HelmRepository",
		label: "Flux Helm Repositories",
		legacyLabels: ["Flux HelmRepositories"],
		family: "sources",
	},
	{
		kind: "HelmChart",
		label: "Flux Helm Charts",
		legacyLabels: ["Flux HelmCharts"],
		family: "sources",
	},
	{
		kind: "Bucket",
		label: "Flux Buckets",
		legacyLabels: ["Flux Buckets"],
		family: "sources",
	},
	{
		kind: "Kustomization",
		label: "Flux Kustomizations",
		legacyLabels: ["Flux Kustomizations"],
		family: "workloads",
	},
	{
		kind: "HelmRelease",
		label: "Flux Helm Releases",
		legacyLabels: ["Flux HelmReleases"],
		family: "workloads",
	},
	{
		kind: "Provider",
		label: "Flux Providers",
		legacyLabels: ["Flux Providers"],
		family: "notifications",
	},
	{
		kind: "Alert",
		label: "Flux Alerts",
		legacyLabels: ["Flux Alerts"],
		family: "notifications",
	},
	{
		kind: "Receiver",
		label: "Flux Receivers",
		legacyLabels: ["Flux Receivers"],
		family: "notifications",
	},
	{
		kind: "ImageRepository",
		label: "Flux Image Repositories",
		legacyLabels: ["Flux ImageRepositories"],
		family: "imageAutomation",
	},
	{
		kind: "ImagePolicy",
		label: "Flux Image Policies",
		legacyLabels: ["Flux ImagePolicies"],
		family: "imageAutomation",
	},
	{
		kind: "ImageUpdateAutomation",
		label: "Flux Image Update Automations",
		legacyLabels: ["Flux ImageUpdateAutomations"],
		family: "imageAutomation",
	},
];

export const FLUX_FAMILIES: FluxFamily[] = [
	{
		key: "sources",
		label: "Sources",
		groupId: "gitops:flux:sources",
		kinds: FLUX_NAV_KINDS.filter((kind) => kind.family === "sources"),
	},
	{
		key: "workloads",
		label: "Workloads",
		groupId: "gitops:flux:workloads",
		kinds: FLUX_NAV_KINDS.filter((kind) => kind.family === "workloads"),
	},
	{
		key: "notifications",
		label: "Notifications",
		groupId: "gitops:flux:notifications",
		kinds: FLUX_NAV_KINDS.filter((kind) => kind.family === "notifications"),
	},
	{
		key: "imageAutomation",
		label: "Image Automation",
		groupId: "gitops:flux:image-automation",
		kinds: FLUX_NAV_KINDS.filter(
			(kind) => kind.family === "imageAutomation",
		),
	},
];

export const FLUX_KIND_LABELS: Record<string, string> = FLUX_NAV_KINDS.reduce(
	(labels, kind) => {
		labels[kind.label] = kind.kind;
		for (const legacyLabel of kind.legacyLabels) {
			labels[legacyLabel] = kind.kind;
		}
		return labels;
	},
	{} as Record<string, string>,
);

export function fluxKindDefinitionFromLabel(
	label: string | null,
): FluxNavKind | null {
	if (!label) return null;
	const kindName = FLUX_KIND_LABELS[label];
	if (!kindName) return null;
	return FLUX_NAV_KINDS.find((kind) => kind.kind === kindName) ?? null;
}

export function fluxKindFromLabel(
	label: string | null,
	kinds: FluxResourceKind[],
): FluxResourceKind | null {
	const kindName = label ? FLUX_KIND_LABELS[label] : null;
	if (!kindName) return null;
	return kinds.find((candidate) => candidate.kind === kindName) ?? null;
}

export function isFluxKindLabel(label: string | null): boolean {
	return Boolean(label && FLUX_KIND_LABELS[label]);
}

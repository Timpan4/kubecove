import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceKind,
	FluxResourceSummary,
} from "@/lib/types";
import {
	ARGO_NAV_KINDS,
	FLUX_FAMILIES,
	fluxKindDefinitionFromLabel,
	type FluxFamilyKey,
} from "./gitops-nav";

export type GitOpsOverviewFilterKey =
	| "argo:applications"
	| "argo:applicationSets"
	| "argo:appProjects"
	| `flux:${string}`;

export interface GitOpsOverviewFilter {
	key: GitOpsOverviewFilterKey;
	provider: "Argo CD" | "Flux";
	group: string;
	label: string;
	count: number;
	installedKinds?: number;
	disabled?: boolean;
}

export interface GitOpsOverviewFiltersInput {
	argoDetected: boolean;
	showUnavailableArgo: boolean;
	apps: ArgoApplicationSummary[];
	appsets: ArgoApplicationSetSummary[];
	projects: ArgoAppProjectSummary[];
	fluxDetected: boolean;
	showUnavailableFlux: boolean;
	fluxKinds: FluxResourceKind[];
	fluxRows: FluxResourceSummary[];
}

const DEFAULT_KIND_ORDER = [
	"argo:applications",
	"flux:Kustomization",
	"flux:HelmRelease",
] as const;

function fluxCount(rows: FluxResourceSummary[], kind: string): number {
	return rows.filter((row) => row.resourceKind.kind === kind).length;
}

export function fluxFamilyInstalledKindCount(
	kinds: FluxResourceKind[],
	familyKey: FluxFamilyKey,
): number {
	const family = FLUX_FAMILIES.find((item) => item.key === familyKey);
	if (!family) return 0;
	const familyKinds = new Set(family.kinds.map((kind) => kind.kind));
	return kinds.filter((kind) => familyKinds.has(kind.kind)).length;
}

export function buildGitOpsOverviewFilters({
	argoDetected,
	showUnavailableArgo,
	apps,
	appsets,
	projects,
	fluxDetected,
	showUnavailableFlux,
	fluxKinds,
	fluxRows,
}: GitOpsOverviewFiltersInput): GitOpsOverviewFilter[] {
	const filters: GitOpsOverviewFilter[] = [];
	if (argoDetected || showUnavailableArgo) {
		filters.push(
			{
				key: "argo:applications",
				provider: "Argo CD",
				group: "Argo CD",
				label: ARGO_NAV_KINDS[0].label,
				count: apps.length,
				disabled: !argoDetected,
			},
			{
				key: "argo:applicationSets",
				provider: "Argo CD",
				group: "Argo CD",
				label: ARGO_NAV_KINDS[1].label,
				count: appsets.length,
				disabled: !argoDetected,
			},
			{
				key: "argo:appProjects",
				provider: "Argo CD",
				group: "Argo CD",
				label: ARGO_NAV_KINDS[2].label,
				count: projects.length,
				disabled: !argoDetected,
			},
		);
	}
	if (fluxDetected || showUnavailableFlux) {
		for (const family of FLUX_FAMILIES) {
			for (const kind of family.kinds) {
				const installed = fluxKinds.some(
					(candidate) => candidate.kind === kind.kind,
				);
				if (!installed && fluxDetected) continue;
				filters.push({
					key: `flux:${kind.kind}`,
					provider: "Flux",
					group: family.label,
					label: kind.label,
					count: fluxCount(fluxRows, kind.kind),
					installedKinds: fluxFamilyInstalledKindCount(fluxKinds, family.key),
					disabled: !fluxDetected || !installed,
				});
			}
		}
	}
	return filters;
}

export function chooseDefaultGitOpsFilter(
	filters: GitOpsOverviewFilter[],
): GitOpsOverviewFilterKey | null {
	for (const key of DEFAULT_KIND_ORDER) {
		const preferred = filters.find(
			(filter) => filter.key === key && !filter.disabled && filter.count > 0,
		);
		if (preferred) return preferred.key;
	}
	return (
		filters.find((filter) => !filter.disabled && filter.count > 0)?.key ??
		filters.find((filter) => !filter.disabled)?.key ??
		null
	);
}

export function fluxKindLabelFromFilterKey(
	filterKey: GitOpsOverviewFilterKey,
): string | null {
	if (!filterKey.startsWith("flux:")) return null;
	const kind = filterKey.slice("flux:".length);
	return (
		FLUX_FAMILIES.flatMap((family) => family.kinds).find(
			(candidate) => candidate.kind === kind,
		)?.label ?? null
	);
}

export function fluxResourceKindFromFilterKey(
	filterKey: GitOpsOverviewFilterKey,
	kinds: FluxResourceKind[],
): FluxResourceKind | null {
	const label = fluxKindLabelFromFilterKey(filterKey);
	if (!label) return null;
	const definition = fluxKindDefinitionFromLabel(label);
	if (!definition) return null;
	return kinds.find((kind) => kind.kind === definition.kind) ?? null;
}

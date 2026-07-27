import type { GitOpsOwnerSummary, ResourceSummary } from "./types";

export interface GitOpsOwnershipPresentation {
	filterKey: string;
	filterKeys: string[];
	label: string;
	ownerName: string;
	searchTerms: string[];
	details: Array<{ label: string; value: string }>;
}

export interface GitOpsOwnershipFilter {
	key: string;
	label: string;
}

type ProviderAdapter = (
	owner: GitOpsOwnerSummary,
) => Omit<GitOpsOwnershipPresentation, "filterKey" | "filterKeys" | "searchTerms" | "details"> | null;

const argoAdapter: ProviderAdapter = (owner) => {
	if (owner.provider !== "argo" || owner.kind !== "Application") return null;
	return {
		ownerName: owner.name,
		label: `Owned by Argo CD: ${owner.name}`,
	};
};

const fluxAdapter: ProviderAdapter = (owner) => {
	if (
		owner.provider !== "flux" ||
		(owner.kind !== "Kustomization" && owner.kind !== "HelmRelease")
	) {
		return null;
	}
	const ownerName = owner.namespace
		? `${owner.namespace}/${owner.name}`
		: owner.name;
	return {
		ownerName,
		label: `Owned by Flux ${owner.kind}: ${ownerName}`,
	};
};

const PROVIDER_ADAPTERS = [argoAdapter, fluxAdapter];

export function gitOpsOwnership(
	resource: Pick<ResourceSummary, "gitOpsOwner" | "argoApp">,
): GitOpsOwnershipPresentation | null {
	const owner = resource.gitOpsOwner ?? (resource.argoApp?.trim()
		? {
				provider: "argo",
				kind: "Application",
				name: resource.argoApp.trim(),
				confidence: "metadata",
				provenance: "argocd-name-annotation",
			}
		: undefined);
	if (!owner?.name.trim()) return null;
	const adapted = PROVIDER_ADAPTERS.map((adapter) => adapter(owner)).find(Boolean);
	if (!adapted) return null;

	const partial = owner.partial ?? owner.confidence !== "inventory";
	const filterKey = [
		owner.provider,
		owner.kind,
		owner.namespace ?? "",
		owner.name,
	].join(":");
	const label = partial ? `${adapted.label} (partial evidence)` : adapted.label;
	const evidence = evidenceLabel(owner);

	return {
		...adapted,
		filterKey,
		filterKeys:
			owner.provider === "argo" && owner.kind === "Application"
				? [filterKey, owner.name]
				: [filterKey],
		label,
		searchTerms: [
			owner.provider,
			owner.kind,
			owner.name,
			owner.namespace,
			owner.confidence,
			owner.provenance,
		].filter((value): value is string => Boolean(value)),
		details: [
			{ label: "GitOps", value: adapted.label },
			{ label: "Evidence", value: partial ? `${evidence} (partial)` : evidence },
		],
	};
}

export function gitOpsOwnershipFilters(
	resources: ResourceSummary[],
): GitOpsOwnershipFilter[] {
	const filters = new Map<string, string>();
	for (const resource of resources) {
		const ownership = gitOpsOwnership(resource);
		if (ownership) filters.set(ownership.filterKey, ownership.label);
	}
	return Array.from(filters, ([key, label]) => ({ key, label })).sort((a, b) =>
		a.label.localeCompare(b.label),
	);
}

export function gitOpsOwnershipGroupLabel(resource: ResourceSummary): string {
	return gitOpsOwnership(resource)?.label ??
		(resource.helmRelease ? `Helm release: ${resource.helmRelease}` :
			resource.gitOpsOwnershipPartial
				? "No GitOps ownership evidence (partial coverage)"
				: "No GitOps ownership evidence");
}

export function gitOpsOwnershipFilterValue(filter: string): string {
	const [provider, kind, , name] = filter.split(":");
	return provider && kind && name ? name : filter;
}

export function inheritGitOpsOwnership(resources: ResourceSummary[]): ResourceSummary[] {
	const ownerKeys = new Set<string>();
	for (const resource of resources) {
		if (resource.ownerRef) {
			ownerKeys.add(`${resource.namespace ?? ""}/${resource.ownerRef}`);
		}
	}
	if (ownerKeys.size === 0) return resources;

	const resourcesByName = new Map<string, ResourceSummary>();
	for (const resource of resources) {
		const key = `${resource.namespace ?? ""}/${resource.name}`;
		if (ownerKeys.has(key)) resourcesByName.set(key, resource);
	}
	if (resourcesByName.size === 0) return resources;

	return resources.map((resource) => {
		if (gitOpsOwnership(resource)) return resource;
		const seen = new Set<ResourceSummary>([resource]);
		let current: ResourceSummary | undefined = resource;
		while (current?.ownerRef) {
			const owner = resourcesByName.get(
				`${current.namespace ?? ""}/${current.ownerRef}`,
			);
			if (!owner || seen.has(owner)) break;
			if (gitOpsOwnership(owner)) {
				return { ...resource, gitOpsOwner: owner.gitOpsOwner };
			}
			seen.add(owner);
			current = owner;
		}
		return resource;
	});
}

function evidenceLabel(owner: GitOpsOwnerSummary): string {
	switch (owner.provenance) {
		case "argocd-name-annotation":
			return "Argo CD name annotation";
		case "argocd-tracking-annotation":
			return "Argo CD tracking annotation";
		case "argocd-application-label":
			return "Argo CD Application label";
		case "flux-kustomization-labels":
			return "Flux Kustomization labels";
		case "flux-helmrelease-labels":
			return "Flux HelmRelease labels";
		default:
			return owner.confidence === "inventory"
				? "Flux inventory"
				: `${owner.confidence} metadata`;
	}
}

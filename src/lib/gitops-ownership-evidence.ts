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

// In a view of several contexts, owners and Helm releases belong to their row's
// context: the same Argo CD Application in two clusters is two groups and two
// filters. GPUI: `in_context` and `OwnerRef::filter_key` in `gpui/src/model.rs`.
const inContextLabel = (label: string, resource: Pick<ResourceSummary, "cluster">, multiContext: boolean) =>
	multiContext ? `${label} · ${resource.cluster}` : label;
const inContextKey = (key: string, resource: Pick<ResourceSummary, "cluster">, multiContext: boolean) =>
	multiContext ? `${resource.cluster}:${key}` : key;

/** The owner filter keys a row matches. The Argo CD Application name stays a key in every context, for filters set by name. */
export function gitOpsOwnershipFilterKeys(
	resource: Pick<ResourceSummary, "cluster">,
	ownership: GitOpsOwnershipPresentation,
	multiContext = false,
): string[] {
	if (!multiContext) return ownership.filterKeys;
	return ownership.filterKeys.map((key) =>
		key === ownership.filterKey ? inContextKey(key, resource, true) : key,
	);
}

export function gitOpsOwnershipFilters(
	resources: ResourceSummary[],
	multiContext = false,
): GitOpsOwnershipFilter[] {
	const filters = new Map<string, string>();
	for (const resource of resources) {
		const ownership = gitOpsOwnership(resource);
		if (ownership)
			filters.set(
				inContextKey(ownership.filterKey, resource, multiContext),
				inContextLabel(ownership.label, resource, multiContext),
			);
	}
	return Array.from(filters, ([key, label]) => ({ key, label })).sort((a, b) =>
		a.label.localeCompare(b.label),
	);
}

export function gitOpsOwnershipGroupLabel(resource: ResourceSummary, multiContext = false): string {
	return gitOpsOwnershipGroup(resource, multiContext).label;
}

/// Rank orders ownership groups: GitOps-owned, then Helm, then no evidence.
export interface GitOpsOwnershipGroup {
	label: string;
	rank: number;
}

export function gitOpsOwnershipGroup(resource: ResourceSummary, multiContext = false): GitOpsOwnershipGroup {
	const ownership = gitOpsOwnership(resource);
	if (ownership) return { label: inContextLabel(ownership.label, resource, multiContext), rank: 0 };
	if (resource.helmRelease)
		return { label: inContextLabel(`Helm release: ${resource.helmRelease}`, resource, multiContext), rank: 1 };
	return {
		label: resource.gitOpsOwnershipPartial
			? "No GitOps ownership evidence (partial coverage)"
			: "No GitOps ownership evidence",
		rank: 2,
	};
}

/** The owner name of a filter key, with or without the `cluster:` prefix of a multi-context view. */
export function gitOpsOwnershipFilterValue(filter: string): string {
	const parts = filter.split(":");
	const [provider, kind, , name] = parts.slice(-4);
	return parts.length >= 4 && provider && kind && name ? name : filter;
}

const inheritedRows = new WeakMap<ResourceSummary, { ownerKey: string | undefined; row: ResourceSummary }>();

// Owners are matched within one cluster, so a Pod never inherits a same-named owner from another context.
const objectKey = (cluster: string, namespace: string | null | undefined, name: string) =>
	JSON.stringify([cluster, namespace ?? "", name]);

export function inheritGitOpsOwnership(resources: ResourceSummary[]): ResourceSummary[] {
	const ownerKeys = new Set<string>();
	for (const resource of resources) {
		if (resource.ownerRef) {
			ownerKeys.add(objectKey(resource.cluster, resource.namespace, resource.ownerRef));
		}
	}
	if (ownerKeys.size === 0) return resources;

	const resourcesByName = new Map<string, ResourceSummary>();
	for (const resource of resources) {
		const key = objectKey(resource.cluster, resource.namespace, resource.name);
		if (ownerKeys.has(key)) resourcesByName.set(key, resource);
	}
	if (resourcesByName.size === 0) return resources;

	return resources.map((resource) => {
		if (gitOpsOwnership(resource)) return resource;
		const seen = new Set<ResourceSummary>([resource]);
		let current: ResourceSummary | undefined = resource;
		while (current?.ownerRef) {
			const owner = resourcesByName.get(
				objectKey(current.cluster, current.namespace, current.ownerRef),
			);
			if (!owner || seen.has(owner)) break;
			if (gitOpsOwnership(owner)) {
				const ownerKey = JSON.stringify(owner.gitOpsOwner);
				const cached = inheritedRows.get(resource);
				if (cached && cached.ownerKey === ownerKey) return cached.row;
				const row = { ...resource, gitOpsOwner: owner.gitOpsOwner };
				inheritedRows.set(resource, { ownerKey, row });
				return row;
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

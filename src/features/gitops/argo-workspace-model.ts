import type { ArgoManagedResource } from "@/lib/gitops-types";

export interface ArgoResourceCounts {
	total: number;
	needsSync: number;
	current: number;
	healthy: number;
	degraded: number;
	progressing: number;
	prune: number;
}

export type ArgoResourceFilter =
	| "none"
	| "allManaged"
	| "needsSync"
	| "healthy"
	| "degraded"
	| "progressing"
	| "prune";

type ArgoResourceIdentity = Pick<ArgoManagedResource, "group" | "kind" | "namespace" | "name"> & {
	apiVersion?: string | null;
};

export function argoResourceCounts(resources: ArgoManagedResource[]): ArgoResourceCounts {
	const total = resources.length;
	const current = resources.filter(
		(resource) => normalized(resource.status) === "synced",
	).length;
	return {
		total,
		needsSync: total - current,
		current,
		healthy: resources.filter((resource) => normalized(resource.health) === "healthy").length,
		degraded: resources.filter((resource) => normalized(resource.health) === "degraded").length,
		progressing: resources.filter((resource) => normalized(resource.health) === "progressing").length,
		prune: resources.filter((resource) => resource.requiresPruning === true).length,
	};
}

export function argoResourceIdentityKey(resource: ArgoResourceIdentity): string | null {
	const kind = normalized(resource.kind);
	const name = normalized(resource.name);
	if (!kind || !name) return null;
	return `${normalizedGroup(resource)}:${kind}:${normalized(resource.namespace)}:${name}`;
}

export function argoResourceMatchesFilter(
	resource: ArgoManagedResource,
	filter: ArgoResourceFilter,
): boolean {
	if (filter === "none" || filter === "allManaged") return true;
	if (filter === "needsSync") return normalized(resource.status) !== "synced";
	if (filter === "prune") return resource.requiresPruning === true;
	return normalized(resource.health) === filter;
}

export function filterWorkspaceResourcesByArgo<T extends ArgoResourceIdentity>(
	resources: T[],
	managedResources: ArgoManagedResource[],
	filter: ArgoResourceFilter,
): T[] {
	if (filter === "none") return resources;
	const managedKeys = new Set(
		managedResources
			.filter((resource) => argoResourceMatchesFilter(resource, filter))
			.map(argoResourceIdentityKey)
			.filter((key): key is string => key !== null),
	);
	return resources.filter((resource) => {
		const key = argoResourceIdentityKey(resource);
		return key !== null && managedKeys.has(key);
	});
}

function normalizedGroup(resource: ArgoResourceIdentity): string {
	if (resource.group !== undefined && resource.group !== null) return normalized(resource.group);
	const apiVersion = resource.apiVersion;
	return normalized(apiVersion?.includes("/") ? apiVersion.split("/", 1)[0] : "");
}

function normalized(value: string | null | undefined): string {
	return value?.trim().toLowerCase() ?? "";
}

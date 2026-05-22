import type { HelmReleaseSummary, ResourceSummary } from "@/lib/types";

export function helmReleaseKey(release: HelmReleaseSummary): string {
	return `${release.cluster}:${release.namespace}:${release.name}:${release.storageKind}:${release.storageName}`;
}

export function helmStatusTone(status: string | undefined) {
	if (status === "deployed" || status === "superseded") return "success";
	if (status === "failed" || status === "uninstalled") return "error";
	if (status === "pending-install" || status === "pending-upgrade") return "warning";
	return "neutral";
}

export function groupHelmReleasesByNamespace(releases: HelmReleaseSummary[]) {
	const grouped = new Map<string, HelmReleaseSummary[]>();
	for (const release of releases) {
		const namespaceReleases = grouped.get(release.namespace) ?? [];
		namespaceReleases.push(release);
		grouped.set(release.namespace, namespaceReleases);
	}

	return [...grouped.entries()]
		.sort(([left], [right]) => left.localeCompare(right))
		.map(([namespace, namespaceReleases]) => ({
			namespace,
			releases: namespaceReleases.sort((left, right) =>
				left.name.localeCompare(right.name),
			),
		}));
}

export function resourcesOwnedByHelmRelease(
	resources: ResourceSummary[],
	release: Pick<HelmReleaseSummary, "name" | "namespace">,
) {
	return resources.filter(
		(resource) =>
			resource.namespace === release.namespace &&
			resource.helmRelease === release.name,
	);
}

export function helmReleaseResourceLabel(resource: ResourceSummary): string {
	return `${resource.kind}/${resource.name}`;
}

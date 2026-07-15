import type { HelmReleaseSummary, ResourceSummary } from "@/lib/types";
import type {
	HelmReconciliationResource,
	HelmReconciliationStatus,
} from "@/lib/types";

export function helmReleaseKey(release: HelmReleaseSummary): string {
	return `${release.cluster}:${release.namespace}:${release.name}:${release.storageKind}:${release.storageName}`;
}

export function helmStatusTone(status: string | undefined) {
	if (status === "deployed" || status === "superseded") return "success";
	if (status === "failed" || status === "uninstalled") return "error";
	if (status?.startsWith("pending-")) return "warning";
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

export function findHelmReleaseTarget(
	releases: HelmReleaseSummary[],
	target: { name: string; namespace?: string | null } | null | undefined,
): HelmReleaseSummary | null {
	if (!target) return null;
	return (
		releases.find(
			(release) =>
				release.name === target.name &&
				(target.namespace == null || release.namespace === target.namespace),
		) ?? null
	);
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

export function helmReconciliationStatusTone(
	status: HelmReconciliationStatus,
) {
	if (status === "tracked") return "success";
	if (status === "unlabeledLive" || status === "labelOnly") return "warning";
	if (status === "missing") return "error";
	return "neutral";
}

export function helmReconciliationStatusLabel(
	status: HelmReconciliationStatus,
): string {
	if (status === "tracked") return "Tracked";
	if (status === "unlabeledLive") return "Unlabeled live";
	if (status === "missing") return "Missing";
	if (status === "labelOnly") return "Label-only";
	return "Unavailable";
}

export function helmReconciliationResourceLabel(
	resource: HelmReconciliationResource,
): string {
	const kind = resource.kind ?? "Unknown";
	const name = resource.name ?? "<unnamed>";
	return `${kind}/${name}`;
}

export function sortHelmReconciliationResources(
	resources: HelmReconciliationResource[],
) {
	return [...resources].sort((left, right) =>
		[
			left.namespace ?? "",
			left.kind ?? "",
			left.name ?? "",
			left.apiVersion ?? "",
		]
			.join(":")
			.localeCompare(
				[
					right.namespace ?? "",
					right.kind ?? "",
					right.name ?? "",
					right.apiVersion ?? "",
				].join(":"),
			),
	);
}

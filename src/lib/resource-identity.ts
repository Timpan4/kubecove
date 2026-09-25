import type { DiscoveredResourceKind, ResourceSummary } from "./types";

/** Exact identity of one listed resource. */
export function resourceKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.apiVersion ?? ""}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

/** Identity without apiVersion, for sources such as topology that may omit it. */
export function looseResourceKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

/**
 * Index of the candidate that is `target`: an exact identity match wins over an
 * earlier match that only agrees without apiVersion. Returns -1 when neither matches.
 */
export function findResourceIndex<T>(
	candidates: readonly T[],
	target: ResourceSummary,
	summaryOf: (candidate: T) => ResourceSummary,
): number {
	const exact = resourceKey(target);
	const loose = looseResourceKey(target);
	let looseIndex = -1;
	for (const [index, candidate] of candidates.entries()) {
		const summary = summaryOf(candidate);
		if (resourceKey(summary) === exact) return index;
		if (looseIndex < 0 && looseResourceKey(summary) === loose) looseIndex = index;
	}
	return looseIndex;
}

/** Query-key part for a discovered kind; empty for built-in kinds. */
export function dynamicKindKey(kind: DiscoveredResourceKind | null): string {
	return kind
		? `${kind.group}/${kind.version}/${kind.kind}/${kind.plural}/${kind.namespaced}`
		: "";
}

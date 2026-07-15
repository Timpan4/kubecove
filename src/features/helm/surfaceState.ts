import type { HelmReleaseSummary } from "@/lib/types";
import { findHelmReleaseTarget, groupHelmReleasesByNamespace, helmReleaseKey } from "./helpers";

export function buildHelmReleaseState(
	releases: HelmReleaseSummary[],
	search: string,
	namespace: string | null = null,
) {
	const term = search.trim().toLowerCase();
	const matchingSearch = term
		? releases.filter((release) =>
				[
					release.name,
					release.namespace,
					release.chart ?? "",
					release.appVersion ?? "",
					release.status ?? "",
					release.storageKind,
					release.storageName,
				]
					.join(" ")
					.toLowerCase()
					.includes(term),
			)
		: releases;
	return {
		filtered: namespace
			? matchingSearch.filter((release) => release.namespace === namespace)
			: matchingSearch,
		groups: groupHelmReleasesByNamespace(releases),
	};
}

export function resolveHelmNamespace(
	releases: HelmReleaseSummary[],
	namespace: string | null,
): string | null {
	return namespace && releases.some((release) => release.namespace === namespace)
		? namespace
		: null;
}

export function resolveTargetHelmRelease(
	releases: HelmReleaseSummary[] | undefined,
	target: { name: string; namespace?: string | null } | null | undefined,
) {
	if (!releases || !target) return null;
	return findHelmReleaseTarget(releases, target);
}

export function selectedHelmReleasePath(release: HelmReleaseSummary | null) {
	return release ? { name: release.name, namespace: release.namespace } : null;
}

export function selectedHelmReleaseExists(
	releases: HelmReleaseSummary[] | undefined,
	selected: HelmReleaseSummary | null,
): boolean {
	return !selected || !releases || releases.some((release) => helmReleaseKey(release) === helmReleaseKey(selected));
}

import type { KubeconfigSourcesSummary } from "@/lib/types";

/** Adds trimmed, non-empty paths once each, in the order the command receives them. */
export function addKubeconfigPathsMock(source: KubeconfigSourcesSummary, paths: readonly string[] = []): KubeconfigSourcesSummary {
	for (const path of paths.map((value) => value.trim())) {
		if (path && !source.paths.some((entry) => entry.path === path)) source.paths = [...source.paths, { path }];
	}
	return { ...source };
}

/** Orders the known paths as listed. Paths the list omits keep their relative order at the end. */
export function reorderKubeconfigPathsMock(source: KubeconfigSourcesSummary, paths: readonly string[] = []): KubeconfigSourcesSummary {
	const remaining = [...source.paths];
	const ordered = [];
	for (const path of paths) {
		const index = remaining.findIndex((entry) => entry.path === path);
		if (index >= 0) ordered.push(...remaining.splice(index, 1));
	}
	source.paths = [...ordered, ...remaining];
	return { ...source };
}

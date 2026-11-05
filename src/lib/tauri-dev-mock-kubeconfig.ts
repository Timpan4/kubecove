import type { KubeconfigSourcesSummary } from "@/lib/types";

/** Adds trimmed, non-empty paths once each, in the order the command receives them. */
export function addKubeconfigPathsMock(source: KubeconfigSourcesSummary, paths: readonly string[] = []): KubeconfigSourcesSummary {
	for (const path of paths.map((value) => value.trim())) {
		if (path && !source.paths.some((entry) => entry.path === path)) source.paths = [...source.paths, { path }];
	}
	return { ...source };
}

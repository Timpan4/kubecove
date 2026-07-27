import type { ResourceSummary } from "@/lib/types";
import { formatResourceGroupLabel, formatResourceTypeGroupLabel } from "./helpers";

export function pageGitOpsGroupCounts(
	rows: ResourceSummary[],
	groupedByGitOps: boolean,
): Map<string, number> {
	const counts = new Map<string, number>();
	if (!groupedByGitOps) return counts;
	for (const row of rows) {
		const label = formatResourceGroupLabel(row);
		counts.set(label, (counts.get(label) ?? 0) + 1);
	}
	return counts;
}

export function pageTypeGroupCounts(
	rows: ResourceSummary[],
	groupedByGitOps: boolean,
): Map<string, number> {
	const counts = new Map<string, number>();
	if (!groupedByGitOps) return counts;
	for (const row of rows) {
		const key = `${formatResourceGroupLabel(row)}::${formatResourceTypeGroupLabel(row)}`;
		counts.set(key, (counts.get(key) ?? 0) + 1);
	}
	return counts;
}

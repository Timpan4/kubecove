import { gitOpsOwnershipGroupLabel } from "@/lib/gitops-ownership-evidence";
import type { ResourceSummary } from "@/lib/types";
import { formatResourceTypeGroupLabel } from "./helpers";

export function pageGitOpsGroupCounts(
	rows: ResourceSummary[],
	groupedByGitOps: boolean,
): Map<string, number> {
	const counts = new Map<string, number>();
	if (!groupedByGitOps) return counts;
	for (const row of rows) {
		const label = gitOpsOwnershipGroupLabel(row);
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
		const key = `${gitOpsOwnershipGroupLabel(row)}::${formatResourceTypeGroupLabel(row)}`;
		counts.set(key, (counts.get(key) ?? 0) + 1);
	}
	return counts;
}

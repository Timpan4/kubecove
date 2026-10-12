import { gitOpsOwnershipGroupLabel } from "@/lib/gitops-ownership-evidence";
import { OTHER_RESOURCE_KINDS } from "@/lib/resource-kind-order";
import type { ResourceSummary } from "@/lib/types";
import { formatResourceTypeGroupLabel } from "./helpers";

// The user's kind order decides group order; kinds it does not list share the
// "other kinds" slot. The column sort orders rows inside groups and ties.
export function kindGroupRank(row: ResourceSummary, kindOrder: readonly string[]): number {
	const index = kindOrder.indexOf(row.kind);
	if (index !== -1) return index;
	const other = kindOrder.indexOf(OTHER_RESOURCE_KINDS);
	return other === -1 ? kindOrder.length : other;
}

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

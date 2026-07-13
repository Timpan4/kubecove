import type { DeploymentRevision } from "@/lib/types";

export type DeploymentRevisionViewState = "loading" | "error" | "empty" | "single" | "compare";

export function deploymentRevisionViewState(
	isPending: boolean,
	isError: boolean,
	revisions: DeploymentRevision[],
): DeploymentRevisionViewState {
	if (isPending) return "loading";
	if (isError) return "error";
	if (revisions.length === 0) return "empty";
	if (revisions.length === 1) return "single";
	return "compare";
}

export function revisionLabel(revision: DeploymentRevision): string {
	return revision.revision === undefined ? revision.name : `Revision ${revision.revision}`;
}

export function reconcileRevisionSelection(
	revisions: DeploymentRevision[],
	selectedName: string | null,
	comparisonName: string | null,
): { selectedName: string | null; comparisonName: string | null } {
	const selected = revisions.some((revision) => revision.name === selectedName)
		? selectedName
		: (revisions[0]?.name ?? null);
	const comparison = revisions.some(
		(revision) => revision.name === comparisonName && revision.name !== selected,
	)
		? comparisonName
		: (revisions.find((revision) => revision.name !== selected)?.name ?? null);
	return { selectedName: selected, comparisonName: comparison };
}

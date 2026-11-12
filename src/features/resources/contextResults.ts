import { messageFromFriendlyError } from "@/lib/friendly-errors";
import type { ResourceListVersion, ResourceScopeResult } from "@/lib/types";
import { WorkspaceResourceLoadError } from "@/features/workspaces/query";

// Failure modes the merge must not have:
// 1. Rows follow read completion order instead of workspace context rank.
// 2. The same object in two contexts collapses into one row.
// 3. One failed context hides the other contexts' rows or turns into a page error.
// 4. A context whose refresh fails drops the rows it showed before.
// 5. Every context failing shows rows, or an error that omits a context.
// 6. A context still loading holds back the rows of contexts that answered,
//    or the table hides that the context is still loading.
// 7. A context showing placeholder rows hands their list versions to its watches.
// 8. One context keeps another context's error or versions.

/** One context's scope read as the browser's query reports it. */
export interface ContextScopeRead {
	clusterContext: string;
	data: ResourceScopeResult | undefined;
	error: unknown;
	isError: boolean;
	/** No rows yet, not even placeholder rows. */
	isPending: boolean;
	isPlaceholderData: boolean;
}

/** A context whose read failed. `stale` means its earlier rows are still shown. */
export interface ContextReadFailure {
	clusterContext: string;
	message: string;
	stale: boolean;
}

export interface MergedScopeRead {
	data: ResourceScopeResult | undefined;
	contextFailures: ContextReadFailure[];
	/** Contexts with no rows yet while other contexts' rows show, in rank order. */
	loadingContexts: string[];
	/** Contexts whose rows the table shows, settled, placeholder or stale, in rank order. */
	shownContexts: string[];
	/** Each context's settled list versions, or null while its shown rows are placeholders or loading. */
	versions: ReadonlyMap<string, ResourceListVersion[] | null>;
	isPending: boolean;
	isSuccess: boolean;
	isError: boolean;
	isPlaceholderData: boolean;
	error: unknown;
}

/** Merges per-context scope reads, given in workspace context rank order, for display. */
export function mergeContextResults(reads: readonly ContextScopeRead[]): MergedScopeRead {
	const versions = new Map<string, ResourceListVersion[] | null>();
	const contextFailures: ContextReadFailure[] = [];
	const withData: ResourceScopeResult[] = [];
	for (const read of reads) {
		const settled = !read.isPending && !read.isPlaceholderData;
		versions.set(read.clusterContext, settled ? read.data?.resourceVersions ?? [] : null);
		if (read.data) withData.push(read.data);
		if (read.isError) {
			contextFailures.push({
				clusterContext: read.clusterContext,
				message: messageFromFriendlyError(read.error),
				stale: read.data !== undefined,
			});
		}
	}
	// The table waits only while no context has answered; a slower context joins when it answers.
	const isPending = reads.length > 0 && reads.every((read) => read.isPending);
	const isError = reads.length > 0 && reads.every((read) => read.isError);
	return {
		data: mergeData(withData),
		// One context keeps its own error; several name every context.
		contextFailures: reads.length > 1 ? contextFailures : [],
		loadingContexts: isPending ? [] : reads.filter((read) => read.isPending).map((read) => read.clusterContext),
		shownContexts: reads.filter((read) => read.data !== undefined).map((read) => read.clusterContext),
		versions,
		isPending,
		isSuccess: !isPending && !isError && reads.length > 0,
		isError,
		isPlaceholderData: reads.some((read) => read.isPlaceholderData),
		error: !isError ? null : reads.length === 1 ? reads[0].error : new WorkspaceResourceLoadError(
			reads.map((read) => ({ clusterContext: read.clusterContext, reason: read.error })),
		),
	};
}

function mergeData(results: ResourceScopeResult[]): ResourceScopeResult | undefined {
	// One result keeps its identity, so row caches keyed by the array still hit.
	if (results.length <= 1) return results[0];
	const merged: ResourceScopeResult = { resources: [], completedRequests: [], resourceVersions: [], failures: [] };
	for (const result of results) {
		for (const row of result.resources) merged.resources.push(row);
		for (const request of result.completedRequests) merged.completedRequests.push(request);
		for (const version of result.resourceVersions) merged.resourceVersions.push(version);
		for (const failure of result.failures) merged.failures.push(failure);
	}
	return merged;
}

/** The notice for contexts still loading while other contexts' rows show. */
export function contextsLoadingNotice(contexts: readonly string[]): string {
	const names = contexts.map((context) => `"${context}"`).join(", ");
	return `Waiting for context${contexts.length > 1 ? "s" : ""} ${names}. Showing resources from the contexts that answered.`;
}

import type { AppError, ResourceListRequest, ResourceSummary } from "./types";

export interface ResourceScopeFailure {
	clusterContext: string;
	request: ResourceListRequest;
	error: AppError;
}

/** The resourceVersion of the list one request's rows came from. */
export interface ResourceListVersion {
	request: ResourceListRequest;
	resourceVersion: string;
}

export interface ResourceScopeResult {
	resources: ResourceSummary[];
	completedRequests: ResourceListRequest[];
	resourceVersions: ResourceListVersion[];
	failures: ResourceScopeFailure[];
}

/** One kind's list with the resourceVersion it came from. A view starts its watch of that kind at this version. */
export interface VersionedList<T> {
	items: T[];
	/** Absent when no list ran, such as when the kind is not installed. */
	resourceVersion?: string;
}

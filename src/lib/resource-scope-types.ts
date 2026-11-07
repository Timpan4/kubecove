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

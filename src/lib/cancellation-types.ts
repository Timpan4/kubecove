export interface CancellableRequest {
	requestId: string;
	cancelScope: string;
}

export interface CancelBackendRequestsResult {
	cancelled: number;
}

export interface CancelWorkspaceRequestsResult {
	cancelledRequests: number;
	cancelledLoads: number;
	clientGeneration: number;
}

export interface CancellableRequest {
	requestId: string;
	cancelScope: string;
}

export interface CancelBackendRequestsResult {
	cancelled: number;
}

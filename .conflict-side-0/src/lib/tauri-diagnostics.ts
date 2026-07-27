import type {
	CancelBackendRequestsResult,
	CancelWorkspaceRequestsResult,
} from "./cancellation-types";
import type { BackendDiagnosticEvent } from "./diagnostics-types";
import type { TauriClient } from "./tauri";

export async function setBackendDiagnosticsEnabled(
	client: TauriClient,
	enabled: boolean,
): Promise<boolean> {
	return client.invoke<boolean>("set_backend_diagnostics_enabled", { enabled });
}

export async function getBackendDiagnostics(
	client: TauriClient,
): Promise<BackendDiagnosticEvent[]> {
	return client.invoke<BackendDiagnosticEvent[]>("get_backend_diagnostics");
}

export async function clearBackendDiagnostics(
	client: TauriClient,
): Promise<void> {
	return client.invoke<void>("clear_backend_diagnostics");
}

export async function cancelBackendRequests(
	client: TauriClient,
	cancelScope: string,
): Promise<CancelBackendRequestsResult> {
	return client.invoke<CancelBackendRequestsResult>("cancel_backend_requests", {
		cancelScope,
	});
}

export async function cancelWorkspaceRequests(
	client: TauriClient,
): Promise<CancelWorkspaceRequestsResult> {
	return client.invoke<CancelWorkspaceRequestsResult>("cancel_workspace_requests");
}

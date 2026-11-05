//! Controlled host responses for the development browser and shared UI tests.
import type { RemoteConnectionSummary } from "./remote-runtime";
import workspace from "./settings-integration-workspace.json";

export type SettingsScript = "choose-kubeconfig" | "choose-workspace" | "cancel-dialog" | "no-update" | "invalid-signature";
let selected: RemoteConnectionSummary | null = null;
let dialogPath: string | null = null;
let workspaceFile: string | null = null;
let exportedWorkspace = "";
export function configureSettingsMock(script: SettingsScript): void {
	if (script === "choose-kubeconfig") dialogPath = "/controlled/parity.config";
	if (script === "cancel-dialog") dialogPath = null;
    workspaceFile = script === "choose-workspace" ? JSON.stringify(workspace) : null;
}
export function selectedWorkspaceFile(): string | null { return workspaceFile; }
export function saveWorkspaceMock(content: string): boolean { exportedWorkspace = content; return true; }
export function workspaceExportMock(): string { return exportedWorkspace; }
export function selectedDialogPath(): string | null { return dialogPath; }
export function remoteMockStatus(): RemoteConnectionSummary | null { return selected; }
export function connectRemoteMock(serverUrl: string): RemoteConnectionSummary {
	selected = { serverUrl, serverVersion: "0.13.0", sessionGraceSeconds: 300, connected: true };
	return selected;
}
export function disconnectRemoteMock(): undefined { selected = null; }

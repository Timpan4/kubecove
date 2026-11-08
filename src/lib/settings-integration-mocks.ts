//! Controlled host responses for the development browser and shared UI tests.

import { setAppUpdateApiForTests } from "@/features/app-updates/store";
import applicationWorkspace from "./overview-application-workspace.json";
import compareWorkspace from "./overview-compare-workspace.json";
import unavailableWorkspace from "./overview-unavailable-workspace.json";
import { setAppUpdatesEnabledForTests } from "./release-channel";
import type { RemoteConnectionSummary } from "./remote-runtime";
import workspace from "./settings-integration-workspace.json";

export type SettingsScript = "choose-kubeconfig" | "choose-workspace" | "choose-unavailable-workspace" | "choose-application-workspace" | "choose-compare-workspace" | "choose-last-export" | "cancel-dialog" | "no-update" | "invalid-signature" | "update-available";
type UpdateScript = "no-update" | "invalid-signature" | "update-available";
/** Page-session storage, so a reloaded page keeps the update script its scenario configured. */
const UPDATE_SCRIPT_KEY = "kubecove-parity-update-script";
let selected: RemoteConnectionSummary | null = null;
let dialogPath: string | null = null;
let workspaceFile: string | null = null;
let exportedWorkspace = "";
export function configureSettingsMock(script: SettingsScript): void {
	if (script === "choose-kubeconfig") dialogPath = "/controlled/parity.config";
	if (script === "cancel-dialog") dialogPath = null;
    workspaceFile = script === "choose-workspace" ? JSON.stringify(workspace) : null;
    if (script === "choose-unavailable-workspace") workspaceFile = JSON.stringify(unavailableWorkspace);
    if (script === "choose-application-workspace") workspaceFile = JSON.stringify(applicationWorkspace);
    if (script === "choose-compare-workspace") workspaceFile = JSON.stringify(compareWorkspace);
    // Reimports the file the last export saved.
    if (script === "choose-last-export") workspaceFile = exportedWorkspace;
	rememberUpdateScript(script);
	if (isUpdateScript(script)) applyUpdateScript(script);
}
function isUpdateScript(script: string | null): script is UpdateScript {
	return script === "no-update" || script === "invalid-signature" || script === "update-available";
}
/** Enables update checks with the API the script names. The app reads the enablement when it mounts. */
function applyUpdateScript(script: UpdateScript): void {
	setAppUpdatesEnabledForTests(true);
	setAppUpdateApiForTests({
		check: async () => script === "no-update" ? null : {
			currentVersion: "0.13.0",
			version: "0.14.0",
			body: script === "update-available" ? "Fixes topology edges." : undefined,
			downloadAndInstall: async () => { throw new Error("Invalid update signature"); },
		},
		relaunch: async () => { throw new Error("Tests cannot relaunch"); },
	});
}
function rememberUpdateScript(script: SettingsScript): void {
	try {
		if (isUpdateScript(script)) sessionStorage.setItem(UPDATE_SCRIPT_KEY, script);
		else sessionStorage.removeItem(UPDATE_SCRIPT_KEY);
	} catch {
		// Storage can be blocked; the script then applies only to the current page.
	}
}
function storedUpdateScript(): string | null {
	try {
		return sessionStorage.getItem(UPDATE_SCRIPT_KEY);
	} catch {
		return null;
	}
}
export function selectedWorkspaceFile(): string | null { return workspaceFile; }
export function saveWorkspaceMock(content: string): boolean { exportedWorkspace = content; return true; }
export function workspaceExportMock(): string { return exportedWorkspace; }
export function selectedDialogPath(): string | null { return dialogPath; }
export function remoteMockStatus(): RemoteConnectionSummary | null { return selected; }
export function connectRemoteMock(serverUrl: string): RemoteConnectionSummary {
	selected = { serverUrl, serverVersion: "0.13.0", sessionGraceSeconds: 300, connected: true, credentialStore: "encryptedFile" };
	return selected;
}
export function disconnectRemoteMock(): undefined { selected = null; }
/** The selected server stops answering: it stays selected, and its commands fail as disconnected. */
export function dropRemoteConnectionMock(): void {
	if (selected) selected = { ...selected, connected: false };
}
// The dev entry awaits this module before mounting the app, so a reloaded page's startup check sees the stored update API.
const storedScript = storedUpdateScript();
if (isUpdateScript(storedScript)) applyUpdateScript(storedScript);

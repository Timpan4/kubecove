//! Controlled host responses for the development browser and shared UI tests.

import { setAppUpdateApiForTests } from "@/features/app-updates/store";
import applicationWorkspace from "./overview-application-workspace.json";
import compareWorkspace from "./overview-compare-workspace.json";
import unavailableWorkspace from "./overview-unavailable-workspace.json";
import { setAppUpdatesEnabledForTests } from "./release-channel";
import type { RemoteAuthMode, RemoteConnectionSummary, RemoteSessionState } from "./remote-runtime";
import workspace from "./settings-integration-workspace.json";

export type SettingsScript = "oidc-server" | "oidc-browser-waits" | "oidc-sign-in-fails" | "oidc-memory-only" | "oidc-store-keeps-sign-in" | "access-server" | "choose-kubeconfig" | "choose-workspace" | "choose-unavailable-workspace" | "choose-application-workspace" | "choose-compare-workspace" | "choose-last-export" | "cancel-dialog" | "no-update" | "invalid-signature" | "update-available" | "update-installs" | "update-download-waits";
const UPDATE_SCRIPTS = ["no-update", "invalid-signature", "update-available", "update-installs", "update-download-waits"] as const;
type UpdateScript = (typeof UPDATE_SCRIPTS)[number];
/** Page-session storage, so a reloaded page keeps the update script its scenario configured. */
const UPDATE_SCRIPT_KEY = "kubecove-parity-update-script";
let selected: RemoteConnectionSummary | null = null;
let remoteScript: SettingsScript | null = null;
/** Servers this app holds an OIDC sign-in for. */
const signIns = new Set<string>();
let dialogPath: string | null = null;
let workspaceFile: string | null = null;
let exportedWorkspace = "";
export function configureSettingsMock(script: SettingsScript): void {
	remoteScript = script;
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
	return UPDATE_SCRIPTS.some((known) => known === script);
}
/** Enables update checks with the API the script names. The app reads the enablement when it mounts. */
function applyUpdateScript(script: UpdateScript): void {
	setAppUpdatesEnabledForTests(true);
	setAppUpdateApiForTests({
		check: async () => script === "no-update" ? null : {
			currentVersion: "0.13.0",
			version: "0.14.0",
			body: script === "update-available" ? "Fixes topology edges." : undefined,
			downloadAndInstall: async (onEvent) => {
				if (script !== "update-installs" && script !== "update-download-waits") throw new Error("Invalid update signature");
				onEvent?.({ event: "Started", data: { contentLength: 100 } });
				if (script === "update-download-waits") {
					// The download stops at 40% and never finishes.
					onEvent?.({ event: "Progress", data: { chunkLength: 40 } });
					return new Promise<void>(() => {});
				}
				onEvent?.({ event: "Progress", data: { chunkLength: 100 } });
				onEvent?.({ event: "Finished" });
			},
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
function liveSummary(serverUrl: string, authMode: RemoteAuthMode): RemoteConnectionSummary {
	return { serverUrl, serverVersion: "0.13.0", sessionGraceSeconds: 300, connected: true, credentialStore: "encryptedFile", authMode, sessionState: "connected", signInMemoryOnly: false, signInNotDeleted: false };
}
/** An empty token selects the server by the sign-in mode the configured script gives it (ADR 0031). */
export function connectRemoteMock(serverUrl: string, token = ""): RemoteConnectionSummary {
	if (token.trim()) {
		selected = liveSummary(serverUrl, "token");
		return selected;
	}
	if (remoteScript === "access-server") throw new Error("Desktop sign-in for Cloudflare Access is not supported. Use a token or OIDC server.");
	if (!remoteScript?.startsWith("oidc-")) throw new Error("This server uses a server token. Enter the token to connect.");
	const held = signIns.has(serverUrl);
	selected = held
		? { ...liveSummary(serverUrl, "oidc"), signInMemoryOnly: remoteScript === "oidc-memory-only" }
		: { ...liveSummary(serverUrl, "oidc"), connected: false, credentialStore: null, sessionState: "signInRequired" };
	return selected;
}
export function disconnectRemoteMock(): undefined { selected = null; }
export function signInRemoteMock(): RemoteConnectionSummary | null {
	if (!selected) return null;
	// The provider denied the sign-in; the server stays selected and signed out.
	if (remoteScript === "oidc-sign-in-fails") throw new Error("Sign-in was not completed.");
	if (remoteScript === "oidc-browser-waits") {
		selected = { ...selected, connected: false, sessionState: "waitingForBrowser" };
	} else {
		signIns.add(selected.serverUrl);
		selected = { ...selected, connected: true, sessionState: "connected", signInMemoryOnly: remoteScript === "oidc-memory-only", signInNotDeleted: false };
	}
	return selected;
}
export function cancelSignInRemoteMock(): undefined {
	if (selected?.sessionState === "waitingForBrowser") selected = { ...selected, sessionState: "signInRequired" };
}
export function signInLinkRemoteMock(): string | null {
	return selected?.sessionState === "waitingForBrowser" ? "http://127.0.0.1:43117/api/v1/oidc/native/start?redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Fkubecove%2Fcallback" : null;
}
export function startNewSessionRemoteMock(): RemoteConnectionSummary {
	if (!selected) throw new Error("Select a remote KubeCove server first.");
	const held = selected.authMode === "token" || signIns.has(selected.serverUrl);
	selected = { ...selected, connected: held, sessionState: held ? "connected" : "signInExpired" };
	return selected;
}
export function signOutRemoteMock(): RemoteConnectionSummary {
	if (!selected) throw new Error("Select a server that uses single sign-on first.");
	signIns.delete(selected.serverUrl);
	// The credential store refuses to delete the sign-in.
	const signInNotDeleted = remoteScript === "oidc-store-keeps-sign-in";
	selected = { ...selected, connected: false, sessionState: "signInRequired", signInMemoryOnly: false, signInNotDeleted };
	return selected;
}
/** The state a server signal leaves the selected remote in. Commands fail unless it is still reconnecting. */
export function remoteSignalMock(sessionState: RemoteSessionState): void {
	if (!selected) return;
	if (sessionState === "signInExpired") signIns.delete(selected.serverUrl);
	selected = { ...selected, connected: sessionState === "reconnecting", sessionState };
}
/** The selected server stops answering: it stays selected, its commands fail as disconnected, and the grace window has elapsed. */
export function dropRemoteConnectionMock(): void {
	remoteSignalMock("sessionExpired");
}
// The dev entry awaits this module before mounting the app, so a reloaded page's startup check sees the stored update API.
const storedScript = storedUpdateScript();
if (isUpdateScript(storedScript)) applyUpdateScript(storedScript);

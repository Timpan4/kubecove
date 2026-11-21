import { writable } from "svelte/store";

// The browser parity driver exercises the native client's remote settings with fixtures.
async function invoke<T>(command: string, args?: object): Promise<T> {
	if (process.env.KUBECOVE_PUBLIC_DEV_MOCKS === "true") {
		const { createDevMockTauriClient } = await import("./tauri-dev-mocks");
		return createDevMockTauriClient().invoke<T>(command, args);
	}
	throw new Error("Desktop remote controls are only available in browser mocks.");
}


/** How the selected server authenticates this desktop (ADR 0031). */
export type RemoteAuthMode = "token" | "oidc";

/** The selected remote's session state, mirroring `kubecove_host::remote::RemoteSessionState`. */
export type RemoteSessionState =
	| "connected"
	| "signInRequired"
	| "connecting"
	| "waitingForBrowser"
	| "reconnecting"
	| "sessionEnded"
	| "signInExpired"
	| "sessionExpired";

export interface RemoteConnectionSummary {
	serverUrl: string;
	serverVersion: string;
	sessionGraceSeconds: number;
	connected: boolean;
	/** The server's credential store kind, or null when the server does not report one. */
	credentialStore: string | null;
	authMode: RemoteAuthMode;
	sessionState: RemoteSessionState;
	/** Native credential storage is unavailable, so the sign-in lasts until the app closes. */
	signInMemoryOnly: boolean;
	/** Native credential storage could not delete an ended sign-in; the app does not use it again until a new sign-in replaces it. */
	signInNotDeleted: boolean;
}

let desktopRemoteSummary: RemoteConnectionSummary | null = null;
export const desktopRemoteConnection = writable<RemoteConnectionSummary | null>(null);


/** Counts Disconnects, so a connect or session start can tell that one overlapped it. */
let disconnects = 0;
/** Counts state changes from user actions. Status reads do not count, so concurrent reads still publish. */
let actionChanges = 0;

/**
 * Applies the summary `call` returns. A Disconnect during the call may have superseded it, and an invoke
 * cannot be aborted, so the host's state then replaces its result or error.
 */
async function unlessDisconnected(call: () => Promise<RemoteConnectionSummary>): Promise<RemoteConnectionSummary | null> {
	const started = disconnects;
	try {
		const summary = await call();
		if (started === disconnects) {
			setRemoteState(summary);
			return summary;
		}
	} catch (error) {
		if (started === disconnects) throw error;
	}
	return initializeDesktopRemote();
}

export async function connectDesktopRemote(serverUrl: string, token: string): Promise<RemoteConnectionSummary | null> {
	return unlessDisconnected(() => invoke<RemoteConnectionSummary>("remote_connect", { serverUrl, token }));
}

/**
 * Signs in through the system browser. The call returns when the sign-in finishes or is
 * cancelled, so the notice shows the wait until then.
 */
export async function signInDesktopRemote(): Promise<RemoteConnectionSummary | null> {
	if (desktopRemoteSummary) setRemoteState({ ...desktopRemoteSummary, sessionState: "waitingForBrowser", connected: false });
	try {
		const summary = await invoke<RemoteConnectionSummary | null>("remote_sign_in");
		if (summary) setRemoteState(summary);
		else clearRemoteState();
		return summary;
	} catch (error) {
		await initializeDesktopRemote().catch(() => null);
		throw error;
	}
}

export async function cancelDesktopRemoteSignIn(): Promise<void> {
	await invoke("remote_cancel_sign_in");
	await initializeDesktopRemote();
}

/** The pending sign-in's browser link. It carries no secret. */
export async function desktopRemoteSignInLink(): Promise<string | null> {
	return invoke<string | null>("remote_sign_in_link");
}

export async function startNewDesktopRemoteSession(): Promise<RemoteConnectionSummary | null> {
	// The host reports Connecting while the session opens, so the notice shows it now and offers Cancel at once.
	if (desktopRemoteSummary) setRemoteState({ ...desktopRemoteSummary, sessionState: "connecting", connected: false });
	try {
		return await unlessDisconnected(() => invoke<RemoteConnectionSummary>("remote_start_new_session"));
	} catch (error) {
		await initializeDesktopRemote().catch(() => null);
		throw error;
	}
}

/** Revokes the sign-in at the server and forgets it here. The server stays selected. */
export async function signOutDesktopRemote(): Promise<RemoteConnectionSummary> {
	const summary = await invoke<RemoteConnectionSummary>("remote_sign_out");
	setRemoteState(summary);
	return summary;
}

export async function disconnectDesktopRemote(): Promise<void> {
	disconnects += 1;
	try {
		await invoke("remote_disconnect");
	} finally {
		clearRemoteState();
	}
}

/** Reads the host's state. A change from a user action during the read is newer, so the read then publishes nothing. */
export async function initializeDesktopRemote(): Promise<RemoteConnectionSummary | null> {
	const started = actionChanges;
	try {
		const summary = await invoke<RemoteConnectionSummary | null>("remote_connection_status");
		if (started !== actionChanges) return desktopRemoteSummary;
		if (summary) {
			publishRemoteState(summary);
		} else {
			markRemoteDisconnected();
		}
		return summary;
	} catch (error) {
		if (started === actionChanges) markRemoteDisconnected();
		throw error;
	}
}

function setRemoteState(summary: RemoteConnectionSummary): void {
	actionChanges += 1;
	publishRemoteState(summary);
}

function publishRemoteState(summary: RemoteConnectionSummary): void {
	desktopRemoteSummary = summary;
	desktopRemoteConnection.set(summary);
}

function markRemoteDisconnected(): void {
	if (desktopRemoteSummary) {
		publishRemoteState({ ...desktopRemoteSummary, connected: false });
	}
}

function clearRemoteState(): void {
	actionChanges += 1;
	desktopRemoteSummary = null;
	desktopRemoteConnection.set(null);
}

const DISCONNECTED_REMOTE = "The selected remote KubeCove server is disconnected. Reconnect or explicitly disconnect to use a local cluster.";

/** Mirrors the native client's refusal while the selected mock remote is disconnected. */
export function assertSelectedRemoteConnected(): void {
	if (desktopRemoteSummary && !desktopRemoteSummary.connected) throw new Error(DISCONNECTED_REMOTE);
}

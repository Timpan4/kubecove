import { Channel, type InvokeArgs, type InvokeOptions, invoke as nativeInvoke } from "@tauri-apps/api/core";
import { writable } from "svelte/store";
import { getChannelId, shouldUseBrowserDevMocks, type TauriClient } from "./tauri-runtime";
import type { JsonObject, JsonValue } from "./types";

async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
	if (shouldUseBrowserDevMocks()) {
		const { createDevMockTauriClient } = await import("./tauri-dev-mocks");
		return createDevMockTauriClient().invoke<T>(command, args);
	}
	return nativeInvoke<T>(command, args);
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

type RemoteChannel = {
	ready: Promise<void>;
	cleanup?: (unregister?: boolean) => void;
};
const channels = new Map<number, RemoteChannel>();
let desktopRemoteSummary: RemoteConnectionSummary | null = null;
export const desktopRemoteConnection = writable<RemoteConnectionSummary | null>(null);

export function isDesktopRemoteSelected(): boolean {
	return desktopRemoteSummary !== null;
}

export async function connectDesktopRemote(
	serverUrl: string,
	token: string,
): Promise<RemoteConnectionSummary> {
	const summary = await invoke<RemoteConnectionSummary>("remote_connect", {
		serverUrl,
		token,
	});
	setRemoteState(summary);
	return summary;
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

export async function startNewDesktopRemoteSession(): Promise<RemoteConnectionSummary> {
	const summary = await invoke<RemoteConnectionSummary>("remote_start_new_session");
	setRemoteState(summary);
	return summary;
}

/** Revokes the sign-in at the server and forgets it here. The server stays selected. */
export async function signOutDesktopRemote(): Promise<RemoteConnectionSummary> {
	const summary = await invoke<RemoteConnectionSummary>("remote_sign_out");
	setRemoteState(summary);
	if (!summary.connected) clearRemoteChannels();
	return summary;
}

export async function disconnectDesktopRemote(): Promise<void> {
	try {
		await invoke("remote_disconnect");
	} finally {
		clearRemoteState();
	}
}

export async function initializeDesktopRemote(): Promise<RemoteConnectionSummary | null> {
	try {
		const summary = await invoke<RemoteConnectionSummary | null>("remote_connection_status");
		if (summary) {
			setRemoteState(summary);
			if (!summary.connected) clearRemoteChannels();
		} else {
			markRemoteDisconnected();
			clearRemoteChannels();
		}
		return summary;
	} catch (error) {
		markRemoteDisconnected();
		throw error;
	}
}

function setRemoteState(summary: RemoteConnectionSummary): void {
	desktopRemoteSummary = summary;
	desktopRemoteConnection.set(summary);
}

function markRemoteDisconnected(): void {
	if (desktopRemoteSummary) {
		setRemoteState({ ...desktopRemoteSummary, connected: false });
	}
}

function clearRemoteChannels(): void {
	for (const entry of channels.values()) entry.cleanup?.(false);
	channels.clear();
}

function clearRemoteState(): void {
	desktopRemoteSummary = null;
	desktopRemoteConnection.set(null);
	clearRemoteChannels();
}

function rewriteChannelMarkers(serialized: string, used: Set<number>): JsonValue {
	const rewritten = serialized.replace(/"__TAURI_CHANNEL__:(\d+)"/g, (_marker, id: string) => {
		const channelId = Number(id);
		if (!Number.isSafeInteger(channelId) || !channels.has(channelId)) {
			throw new Error("Remote stream channel is no longer subscribed.");
		}
		used.add(channelId);
		return JSON.stringify(`__KUBECOVE_CHANNEL__:${channelId}`);
	});
	const parsed: JsonValue = JSON.parse(rewritten);
	return parsed;
}

function subscribe<T>(onMessage: (message: T) => void): Channel<T> {
	const channel = new Channel<T>(onMessage);
	const channelId = getChannelId(channel);
	const ready = invoke("remote_subscribe_channel", { channelId, channel }).then(() => undefined);
	const originalCleanup = channel["cleanupCallback"];
	const cleanup = (unregister = true) => {
		if (!channels.has(channelId)) return;
		channels.delete(channelId);
		originalCleanup?.();
		if (unregister) void invoke("remote_unsubscribe_channel", { channelId }).catch(() => {});
	};
	channel["cleanupCallback"] = cleanup;
	channels.set(channelId, { ready, cleanup });
	return channel;
}

const DISCONNECTED_REMOTE = "The selected remote KubeCove server is disconnected. Reconnect or explicitly disconnect to use a local cluster.";

/** Refuses a command while the selected remote is disconnected, as the remote client does, so nothing falls back to this laptop. */
export function assertSelectedRemoteConnected(): void {
	if (desktopRemoteSummary && !desktopRemoteSummary.connected) throw new Error(DISCONNECTED_REMOTE);
}

const desktopRemoteClient: TauriClient = {
	createChannel: subscribe,
	invoke: async <Result, Args extends object = object>(
		command: string,
		args?: Args,
		_options?: InvokeOptions,
	): Promise<Result> => {
		if (!desktopRemoteSummary?.connected) {
			throw new Error(DISCONNECTED_REMOTE);
		}
		if (command === "save_workspace_export_json" || command === "pick_workspace_import_json") {
			if (!args) return invoke<Result>(command);
			const localSerialized = JSON.stringify(args);
			if (localSerialized === undefined) throw new Error("Local command arguments could not be serialized.");
			const localArgs: JsonObject = JSON.parse(localSerialized);
			return invoke<Result>(command, localArgs);
		}
		const serializedArgs = JSON.stringify(args ?? {});
		if (serializedArgs === undefined) throw new Error("Remote command arguments could not be serialized.");
		const used = new Set<number>();
		const remoteArgs = rewriteChannelMarkers(serializedArgs, used);
		await Promise.all([...used].map((id) => channels.get(id)?.ready));
		try {
			return await invoke<Result>("remote_invoke", { command, args: remoteArgs });
		} catch (error) {
			try {
				await initializeDesktopRemote();
			} catch {
				// A failed status check preserves the selected host and stream mappings.
			}
			throw error;
		}
	},
};

export function getDesktopRemoteTauriClient(): TauriClient {
	return desktopRemoteClient;
}

import "./browser-token-bootstrap";
import type { Channel, InvokeOptions } from "@tauri-apps/api/core";
import { writable } from "svelte/store";
import { messageFromError } from "./error-redaction";
import type { TauriClient } from "./tauri-runtime";
import type { JsonObject, JsonValue } from "./types";

const SESSION_STORAGE_KEY = "kubecove-browser-resume";
const CHANNEL_MARKER = "__KUBECOVE_CHANNEL__:";

export type BrowserSessionStatus =
	| "checking"
	| "login"
	| "connecting"
	| "ready"
	| "moved";

export interface BrowserSessionState {
	status: BrowserSessionStatus;
	error: string | null;
	serverVersion: string | null;
	credentialStore: string | null;
	sessionGraceSeconds: number;
	channelGap: number;
	authenticationMode: "token" | "cloudflareAccess" | "oidc" | null;
}

const initialState: BrowserSessionState = {
	status: "checking",
	error: null,
	serverVersion: null,
	credentialStore: null,
	sessionGraceSeconds: 0,
	channelGap: 0,
	authenticationMode: null,
};

const sessionState = writable<BrowserSessionState>(initialState);
const sessionSnapshot = { ...initialState };
sessionState.subscribe((state) => Object.assign(sessionSnapshot, state));

interface BrowserStartupTokenGlobal {
	__KUBECOVE_STARTUP_TOKEN__?: string;
}

// SAFETY: browser-token-bootstrap owns this optional property and this module consumes it once.
const startupTokenGlobal = globalThis as typeof globalThis & BrowserStartupTokenGlobal;
let startupToken: string | null = startupTokenGlobal.__KUBECOVE_STARTUP_TOKEN__ ?? null;
delete startupTokenGlobal.__KUBECOVE_STARTUP_TOKEN__;
let resumeToken: string | null = null;
let initialization: Promise<void> | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let disconnectedAt: number | null = null;
let reconnecting = false;
let socket: WebSocket | null = null;
const channelIdSeed = new Uint32Array(1);
globalThis.crypto.getRandomValues(channelIdSeed);
let nextChannelId = channelIdSeed[0] * 2 ** 20;
const channels = new Map<number, BrowserChannel<unknown>>();

function publish(patch: Partial<BrowserSessionState>): void {
	sessionState.update((current) => ({ ...current, ...patch }));
}

function readStoredResumeToken(): string | null {
	try {
		return window.sessionStorage.getItem(SESSION_STORAGE_KEY);
	} catch {
		return null;
	}
}

function saveResumeToken(value: string): void {
	resumeToken = value;
	try {
		window.sessionStorage.setItem(SESSION_STORAGE_KEY, value);
	} catch {
		// The active page can still resume after a socket drop without sessionStorage.
	}
}

function clearResumeToken(): void {
	resumeToken = null;
	try {
		window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
	} catch {
		// Storage can be disabled by browser policy.
	}
}

function apiUrl(path: string): string {
	return new URL(path, window.location.origin).toString();
}

function parseSessionErrorMessage(cause: unknown, fallback: string): string {
	const message = messageFromError(cause);
	return message && message !== "Unknown error" ? message : fallback;
}

class BrowserSessionHttpError extends Error {
	constructor(message: string, readonly status: number) {
		super(message);
	}
}

function isJsonObject(value: JsonValue | undefined): value is JsonObject {
	return value !== null && value !== undefined && Object(value) === value && !Array.isArray(value);
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}

async function postSession(body: { token?: string; resumeToken?: string }): Promise<void> {
	publish({ status: "connecting", error: null });
	const response = await fetch(apiUrl("/api/v1/session"), {
		method: "POST",
		credentials: "same-origin",
		cache: "no-store",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(body),
	});
	const result: JsonValue = await response.json().catch(() => null);
	if (!response.ok && response.status === 401 && sessionSnapshot.authenticationMode === "oidc") {
		returnToOidcSignIn();
		throw new Error("Your sign-in has expired. Sign in again.");
	}
	if (!response.ok) {
		throw new BrowserSessionHttpError(
			parseSessionErrorMessage(result, `Session request failed (${response.status}).`),
			response.status,
		);
	}
	if (!isJsonObject(result) || !isString(result.resumeToken)
		|| !isString(result.serverVersion) || !isString(result.credentialStore)
		|| Number(result.sessionGraceSeconds) !== result.sessionGraceSeconds) {
		throw new Error("The server returned an invalid session response.");
	}
	saveResumeToken(result.resumeToken);
	disconnectedAt = null;
	publish({
		status: "ready",
		error: null,
		serverVersion: result.serverVersion,
		credentialStore: result.credentialStore,
		sessionGraceSeconds: Number(result.sessionGraceSeconds),
	});
	connectSocket();
}

export function subscribeBrowserSession(
	listener: (state: BrowserSessionState) => void,
): () => void {
	return sessionState.subscribe(listener);
}

export function getBrowserSessionState(): BrowserSessionState {
	return { ...sessionSnapshot };
}

export function initializeBrowserSession(): Promise<void> {
	if (initialization) return initialization;
	initialization = (async () => {
		const response = await fetch(apiUrl("/api/v1/auth"), {
			credentials: "same-origin", cache: "no-store",
		});
		const authentication: JsonValue = await response.json().catch(() => null);
		if (!response.ok || !isJsonObject(authentication)
			|| (authentication.mode !== "token" && authentication.mode !== "cloudflareAccess"
				&& authentication.mode !== "oidc")) {
			throw new Error("Could not check server sign-in. Reload to try again.");
		}
		publish({ authenticationMode: authentication.mode });
		if (authentication.mode === "oidc") {
			startupToken = null;
			if (authentication.authenticated !== true) {
				clearResumeToken();
				publish({ status: "login", error: null });
				return;
			}
			resumeToken = readStoredResumeToken();
			try {
				await resumeOrOpenSession(resumeToken);
			} catch (error) {
				if (error instanceof BrowserSessionHttpError && error.status === 409) {
					publish({ status: "moved", error: parseSessionErrorMessage(error, "Could not open a server session.") });
				} else if (error instanceof Error && error.message === "Your sign-in has expired. Sign in again.") {
					publish({ status: "login", error: null });
				} else {
					publish({ status: "login", error: parseSessionErrorMessage(error, "Could not open server session.") });
				}
			}
			return;
		}
		const token = startupToken;
		startupToken = null;
		if (token) {
			try {
				await postSession({ token });
			} catch (error) {
				publish({ status: "login", error: parseSessionErrorMessage(error, "Could not open server session.") });
			}
			return;
		}

		resumeToken = readStoredResumeToken();
		if (!resumeToken) {
			if (authentication.mode === "cloudflareAccess") {
				try {
					await postSession({});
				} catch (error) {
					publish({ status: "login", error: parseSessionErrorMessage(error, "Cloudflare Access sign-in failed. Reload to try again.") });
				}
				return;
			}
			publish({ status: "login", error: null });
			return;
		}
		try {
			await resumeOrOpenSession(resumeToken);
		} catch (error) {
			if (error instanceof BrowserSessionHttpError && error.status === 401) {
				publish({ status: "login", error: null });
				return;
			}
			publish({ status: "moved", error: parseSessionErrorMessage(error, "The session could not be resumed.") });
		}
	})().catch((error) => {
		publish({ status: "login", error: parseSessionErrorMessage(error, "Could not open server session.") });
	});
	return initialization;
}

export async function loginToBrowserServer(token: string): Promise<void> {
	const cleanToken = token.trim();
	if (!cleanToken) {
		publish({ status: "login", error: "Enter the server token." });
		return;
	}
	clearReconnectTimer();
	try {
		await postSession({ token: cleanToken });
	} catch (error) {
		publish({ status: "login", error: parseSessionErrorMessage(error, "Could not open server session.") });
		throw error;
	}
}

// A closed or expired session cannot be resumed, and other sessions never
// block a new one, so a refused resume opens a new session instead.
async function resumeOrOpenSession(resume: string | null): Promise<void> {
	if (resume) {
		try {
			await postSession({ resumeToken: resume });
			return;
		} catch (error) {
			if (!(error instanceof BrowserSessionHttpError && error.status === 409)) throw error;
			clearResumeToken();
		}
	}
	await postSession({});
}

export async function takeOverBrowserSession(): Promise<void> {
	clearReconnectTimer();
	try {
		// A fresh login opens a new session using the cookie or verified Access identity.
		await postSession({});
	} catch (error) {
		if (sessionSnapshot.authenticationMode === "oidc"
			&& error instanceof Error && error.message === "Your sign-in has expired. Sign in again.") {
			publish({ status: "login", error: null });
			return;
		}
		publish({ status: "moved", error: parseSessionErrorMessage(error, "Could not open a server session.") });
		throw error;
	}
}

export async function startBrowserOidcSignIn(): Promise<void> {
	const response = await fetch(apiUrl("/api/v1/oidc/login"), {
		method: "POST",
		credentials: "same-origin",
		cache: "no-store",
	});
	if (!response.ok) throw new Error(`Could not start sign-in (${response.status}).`);
	const result: JsonValue = await response.json().catch(() => null);
	if (!isJsonObject(result) || !isString(result.authorizationUrl)) {
		throw new Error("The server returned an invalid sign-in response.");
	}
	let authorizationUrl: URL;
	try {
		authorizationUrl = new URL(result.authorizationUrl);
	} catch {
		throw new Error("The server returned an invalid sign-in response.");
	}
	if (authorizationUrl.protocol !== "https:" && authorizationUrl.protocol !== "http:") {
		throw new Error("The server returned an invalid sign-in response.");
	}
	window.location.assign(authorizationUrl.toString());
}

export async function signOutBrowserSession(): Promise<void> {
	const headers: Record<string, string> = {};
	if (resumeToken) headers["X-Kubecove-Session"] = resumeToken;
	const response = await fetch(apiUrl("/api/v1/oidc/logout"), {
		method: "POST",
		credentials: "same-origin",
		cache: "no-store",
		headers,
	});
	if (!response.ok) throw new Error(`Sign-out failed (${response.status}).`);
	returnToOidcSignIn();
}

function returnToOidcSignIn(): void {
	clearReconnectTimer();
	reconnecting = false;
	disconnectedAt = null;
	if (socket) {
		const current = socket;
		socket = null;
		current.close();
	}
	for (const channel of channels.values()) channel.cleanupCallback();
	channels.clear();
	clearResumeToken();
	publish({ status: "login", error: null, channelGap: 0 });
}

function clearReconnectTimer(): void {
	if (reconnectTimer !== null) clearTimeout(reconnectTimer);
	reconnectTimer = null;
}

function socketUrl(): string {
	const url = new URL("/api/v1/channels", window.location.origin);
	url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
	url.searchParams.set("resumeToken", resumeToken ?? "");
	return url.toString();
}

function connectSocket(): void {
	if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) return;
	if (sessionSnapshot.status !== "ready" || !resumeToken) return;
	const next = new WebSocket(socketUrl());
	socket = next;
	next.onopen = () => {
		if (socket !== next) return;
		reconnecting = false;
		disconnectedAt = null;
		clearReconnectTimer();
		publish({ status: "ready", error: null });
	};
	next.onmessage = (event) => {
		if (socket !== next || !isString(event.data)) return;
		let message: JsonValue;
		try {
			message = JSON.parse(event.data);
		} catch {
			return;
		}
		if (!isJsonObject(message)) return;
		if (message.type === "session_moved") {
			clearReconnectTimer();
			reconnecting = false;
			disconnectedAt = null;
			publish({ status: "moved", error: null });
			next.close();
			return;
		}
		if (message.type === "channel_gap") {
			if (Number.isSafeInteger(message.dropped) && Number(message.dropped) >= 0) {
				publish({ channelGap: sessionSnapshot.channelGap + Number(message.dropped) });
			}
			return;
		}
		if (message.type !== "channel" || !Number.isSafeInteger(message.channelId)) return;
		channels.get(Number(message.channelId))?.onmessage(message.message);
	};
	next.onclose = () => {
		if (socket !== next) return;
		socket = null;
		if (sessionSnapshot.status === "moved" || sessionSnapshot.status !== "ready") return;
		if (disconnectedAt === null) disconnectedAt = Date.now();
		if (sessionSnapshot.authenticationMode === "oidc") {
			void checkOidcAuthenticationAfterSocketClose();
			return;
		}
		publish({ status: "ready", error: "Connection interrupted. Reconnecting to the current session." });
		scheduleReconnect();
	};
	next.onerror = () => {
		// onclose schedules recovery; avoid logging socket URLs, which contain the resume token.
	};
}

async function checkOidcAuthenticationAfterSocketClose(): Promise<void> {
	try {
		const response = await fetch(apiUrl("/api/v1/auth"), {
			credentials: "same-origin",
			cache: "no-store",
		});
		const authentication: JsonValue = await response.json().catch(() => null);
		if (response.ok && isJsonObject(authentication)
			&& authentication.mode === "oidc" && authentication.authenticated === false) {
			returnToOidcSignIn();
			return;
		}
	} catch {
		// A temporary auth-check failure falls back to the existing session grace retry.
	}
	if (sessionSnapshot.status !== "ready") return;
	publish({ status: "ready", error: "Connection interrupted. Reconnecting to the current session." });
	scheduleReconnect();
}

function scheduleReconnect(): void {
	if (reconnecting || reconnectTimer !== null || !resumeToken) return;
	const graceMs = sessionSnapshot.sessionGraceSeconds * 1000;
	const elapsedMs = disconnectedAt === null ? 0 : Date.now() - disconnectedAt;
	if (graceMs <= 0 || elapsedMs >= graceMs) {
		clearResumeToken();
		publish({ status: "login", error: sessionSnapshot.authenticationMode === "cloudflareAccess"
			? "The server session expired. Reload to reconnect through Cloudflare Access."
			: sessionSnapshot.authenticationMode === "oidc"
				? "Your sign-in has expired. Sign in again."
				: "The server session expired. Reconnect with the server token." });
		return;
	}
	reconnecting = true;
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null;
		reconnecting = false;
		connectSocket();
	}, Math.min(1_000, graceMs - elapsedMs));
}

interface BrowserChannel<T> {
	id: number;
	onmessage: (message: T) => void;
	cleanupCallback: () => void;
	unregister: () => Promise<void>;
	toJSON: () => string;
}

function createBrowserChannel<T>(onMessage: (message: T) => void): Channel<T> {
	let active = true;
	const id = nextChannelId++;
	const channel: BrowserChannel<T> = {
		id,
		onmessage: (message) => {
			if (active) onMessage(message);
		},
		cleanupCallback: () => {
			active = false;
			channels.delete(id);
		},
		unregister: async () => {
			active = false;
			channels.delete(id);
			if (!resumeToken || !window.navigator.onLine) return;
			try {
				await fetch(apiUrl(`/api/v1/channels/${id}/close`), {
					method: "POST",
					credentials: "same-origin",
					keepalive: true,
					headers: { "X-Kubecove-Session": resumeToken },
				});
			} catch {
				// The server removes channels when their owner grace window expires.
			}
		},
		toJSON: () => `${CHANNEL_MARKER}${id}`,
	};
	// SAFETY: typed command wrappers choose T to match the Rust channel's serialized payload.
	channels.set(id, channel as BrowserChannel<unknown>);
	// SAFETY: the browser adapter implements the Channel members consumed by typed wrappers and cleanup.
	// @ts-expect-error: Tauri Channel's private native runtime members are replaced by the browser adapter.
	return channel as Channel<T>;
}

let browserClient: TauriClient | null = null;

export function createBrowserTauriClient(): TauriClient {
	if (browserClient) return browserClient;
	browserClient = {
		createChannel: createBrowserChannel,
		invoke: async <T, Args extends object = object>(
			command: string,
			args?: Args,
			_options?: InvokeOptions,
		): Promise<T> => {
			if (sessionSnapshot.status !== "ready" || !resumeToken) {
				throw new Error("Open a KubeCove server session before invoking commands.");
			}
			const response = await fetch(apiUrl(`/api/v1/invoke/${encodeURIComponent(command)}`), {
				method: "POST",
				credentials: "same-origin",
				cache: "no-store",
				headers: {
					"Content-Type": "application/json",
					"X-Kubecove-Session": resumeToken,
				},
				body: JSON.stringify(args ?? {}),
			});
			const result: unknown = await response.json().catch(() => null);
			if (!response.ok) {
				if (response.status === 401 && sessionSnapshot.authenticationMode === "oidc") {
					returnToOidcSignIn();
				}
				const error = new Error(parseSessionErrorMessage(result, `Command ${command} failed (${response.status}).`));
				if (result && Object(result) === result) Object.assign(error, result);
				throw error;
			}
			// SAFETY: typed command wrappers select T from the matching Rust serde result contract.
			return result as T;
		},
	};
	return browserClient;
}

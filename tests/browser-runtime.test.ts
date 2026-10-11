import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import {
	createBrowserTauriClient,
	getBrowserSessionState,
	initializeBrowserSession,
	signOutBrowserSession,
	startBrowserOidcSignIn,
	takeOverBrowserSession,
} from "../src/lib/browser-runtime";

const originalFetch = globalThis.fetch;
const originalWebSocket = globalThis.WebSocket;
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const storage = new Map<string, string>();
let navigation: string | null = null;

class TestWebSocket {
	static instances: TestWebSocket[] = [];
	readyState = 0;
	onopen: (() => void) | null = null;
	onmessage: ((event: { data: string }) => void) | null = null;
	onclose: (() => void) | null = null;
	onerror: (() => void) | null = null;
	closed = false;

	constructor(_url: string) {
		TestWebSocket.instances.push(this);
	}

	close() {
		this.closed = true;
		this.readyState = 3;
	}
}

const browserWindow = {
	location: {
		origin: "https://kubecove.example",
		host: "kubecove.example",
		assign: (url: string) => { navigation = url; },
	},
	navigator: { onLine: true },
	sessionStorage: {
		getItem: (key: string) => storage.get(key) ?? null,
		setItem: (key: string, value: string) => storage.set(key, value),
		removeItem: (key: string) => storage.delete(key),
	},
};
beforeEach(() => {
	Object.defineProperty(globalThis, "window", { configurable: true, value: browserWindow });
	Object.defineProperty(globalThis, "WebSocket", { configurable: true, value: TestWebSocket });
});

type TestResponseBody = Record<string, string | boolean | number>;
type FetchHandler = (...args: Parameters<typeof fetch>) => ReturnType<typeof fetch>;

function setFetch(handler: FetchHandler): void {
	globalThis.fetch = Object.assign(handler, { preconnect: originalFetch.preconnect });
}

function jsonResponse(body: TestResponseBody, status = 200): Response {
	return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

afterEach(() => {
	globalThis.fetch = originalFetch;
	Object.defineProperty(globalThis, "WebSocket", { configurable: true, value: originalWebSocket });
	if (originalWindow) Object.defineProperty(globalThis, "window", originalWindow);
	else Reflect.deleteProperty(globalThis, "window");
});

describe("browser OIDC session lifecycle", () => {
	test("starts sign-in with a cookie POST before navigating to the provider", async () => {
		let request: RequestInit | undefined;
		setFetch(async (input: RequestInfo | URL, init?: RequestInit) => {
			expect(String(input)).toBe("https://kubecove.example/api/v1/oidc/login");
			request = init;
			return jsonResponse({ authorizationUrl: "https://identity.example/authorize?state=fixture" });
		});
		await startBrowserOidcSignIn();
		expect(request).toMatchObject({ method: "POST", credentials: "same-origin", cache: "no-store" });
		expect(navigation).toBe("https://identity.example/authorize?state=fixture");
		navigation = null;
		setFetch(async () => jsonResponse({ authorizationUrl: "javascript:alert(1)" }));
		await expect(startBrowserOidcSignIn()).rejects.toThrow("invalid sign-in response");
		expect(navigation).toBeNull();
	});

	test("opens a new OIDC session after a stale resume and returns expired sign-ins to login", async () => {
		storage.set("kubecove-browser-resume", "stale-resume-token");
		const requests: { url: string; init?: RequestInit }[] = [];
		let sessionRequests = 0;
		let authChecks = 0;
		setFetch(async (input: RequestInfo | URL, init?: RequestInit) => {
			const url = String(input);
			requests.push({ url, init });
			if (url.endsWith("/api/v1/auth")) {
				authChecks++;
				return jsonResponse({ mode: "oidc", authenticated: authChecks === 1 });
			}
			if (url.endsWith("/api/v1/session")) {
				sessionRequests++;
				if (sessionRequests === 1) return jsonResponse({ error: "Session is no longer available." }, 409);
				if (sessionRequests === 4) return jsonResponse({ error: "Unauthorized" }, 401);
				return jsonResponse({ resumeToken: "current-owner-resume", serverVersion: "test", credentialStore: "memory", sessionGraceSeconds: 30 });
			}
			if (url.endsWith("/api/v1/oidc/logout")) return jsonResponse({});
			throw new Error(`Unexpected request to ${url}`);
		});

		await initializeBrowserSession();
		expect(requests[1]?.url).toBe("https://kubecove.example/api/v1/session");
		expect(JSON.parse(String(requests[1]?.init?.body))).toEqual({ resumeToken: "stale-resume-token" });
		expect(requests[2]?.url).toBe("https://kubecove.example/api/v1/session");
		expect(JSON.parse(String(requests[2]?.init?.body))).toEqual({});
		expect(sessionRequests).toBe(2);
		expect(getBrowserSessionState()).toMatchObject({ status: "ready", authenticationMode: "oidc" });
		expect(storage.get("kubecove-browser-resume")).toBe("current-owner-resume");

		let channelMessages = 0;
		const createChannel = createBrowserTauriClient().createChannel;
		if (!createChannel) throw new Error("Browser client must support channels");
		const channel = createChannel<{ value: string }>(() => channelMessages++);
		channel.onmessage({ value: "before" });
		expect(channelMessages).toBe(1);

		await signOutBrowserSession();
		channel.onmessage({ value: "after" });
		expect(channelMessages).toBe(1);
		expect(getBrowserSessionState()).toMatchObject({ status: "login", authenticationMode: "oidc" });
		expect(storage.has("kubecove-browser-resume")).toBe(false);
		expect(requests.at(-1)?.url).toBe("https://kubecove.example/api/v1/oidc/logout");
		expect(requests.at(-1)?.init).toMatchObject({
			method: "POST",
			credentials: "same-origin",
			headers: { "X-Kubecove-Session": "current-owner-resume" },
		});
		expect(TestWebSocket.instances[0]?.closed).toBe(true);

		await takeOverBrowserSession();
		const activeSocket = TestWebSocket.instances.at(-1);
		activeSocket?.onclose?.();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(requests.some((request) => request.url.endsWith("/api/v1/auth"))).toBe(true);
		expect(getBrowserSessionState()).toMatchObject({ status: "login", authenticationMode: "oidc" });
		expect(storage.has("kubecove-browser-resume")).toBe(false);

		await takeOverBrowserSession();
		expect(getBrowserSessionState()).toMatchObject({ status: "login", authenticationMode: "oidc" });
		expect(storage.has("kubecove-browser-resume")).toBe(false);
	});
});

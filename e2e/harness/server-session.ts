import assert from "node:assert/strict";

interface Session {
	resumeToken: string;
	sessionGraceSeconds: number;
	serverVersion: string;
	credentialStore: string;
}

type Frame = { type: string; channelId?: number; message?: { type: string }; error?: { kind: string }; resolvedPodName?: string; resolvedPodPort?: number };

export const channelMarker = (id: number) => `__KUBECOVE_CHANNEL__:${id}`;

// Bun and Node's Undici WebSocket accept headers for native authenticated clients.
const AuthenticatedWebSocket = WebSocket as typeof WebSocket & {
	new (url: URL, options: { headers: Record<string, string> }): WebSocket;
};

export function serverClient(origin: string, token: string) {
	const sockets = new Set<WebSocket>();
	async function post(path: string, body: object, headers: Record<string, string> = {}): Promise<Response> {
		return fetch(`${origin}${path}`, {
			method: "POST",
			headers: { "Content-Type": "application/json", Origin: origin, ...headers },
			body: JSON.stringify(body),
		});
	}

	async function login(resumeToken?: string): Promise<{ session: Session; cookie: string }> {
		const response = await post("/api/v1/session", { token, resumeToken });
		assert.equal(response.status, 200, "Valid login must succeed.");
		const cookie = response.headers.get("set-cookie") ?? "";
		assert.ok(cookie.includes("HttpOnly"), "Session cookie must be HttpOnly");
		assert.ok(cookie.includes("SameSite=Strict"), "Session cookie must be SameSite=Strict");
		const session: Session = await response.json();
		assert.equal(session.sessionGraceSeconds, 5 * 60);
		assert.ok(session.serverVersion);
		assert.ok(["native", "encryptedFile"].includes(session.credentialStore));
		return { session, cookie: cookie.split(";")[0] };
	}

	async function invoke(session: Session, command: string, args: object, cookie?: string): Promise<Response> {
		return post(`/api/v1/invoke/${command}`, args, {
			"X-Kubecove-Session": session.resumeToken,
			...(cookie ? { Cookie: cookie } : { Authorization: `Bearer ${token}` }),
		});
	}

	async function connect(session: Session, onFrame?: (frame: Frame) => void) {
		const url = new URL("/api/v1/channels", origin);
		url.protocol = "ws:";
		url.searchParams.set("resumeToken", session.resumeToken);
		const socket = new AuthenticatedWebSocket(url, { headers: { Authorization: `Bearer ${token}` } });
		socket.binaryType = "arraybuffer";
		sockets.add(socket);
		const queued: Frame[] = [];
		const binary: Uint8Array[] = [];
		let receiver: { resolve: (frame: Frame) => void; reject: (error: Error) => void } | null = null;
		let binaryReceiver: { resolve: (frame: Uint8Array) => void; reject: (error: Error) => void } | null = null;
		socket.addEventListener("message", (event) => {
			if (event.data instanceof ArrayBuffer) {
				const frame = new Uint8Array(event.data);
				if (binaryReceiver) { binaryReceiver.resolve(frame); binaryReceiver = null; }
				else binary.push(frame);
				return;
			}
			const frame: Frame = JSON.parse(event.data);
			if (onFrame) { onFrame(frame); return; }
			if (receiver) { receiver.resolve(frame); receiver = null; }
			else queued.push(frame);
		});
		socket.addEventListener("close", () => {
			sockets.delete(socket);
			receiver?.reject(new Error("Channel socket closed before the expected frame."));
			receiver = null;
			binaryReceiver?.reject(new Error("Tunnel socket closed before the expected bytes."));
			binaryReceiver = null;
		});
		await new Promise<void>((resolveOpen, reject) => {
			socket.addEventListener("open", () => resolveOpen(), { once: true });
			socket.addEventListener("error", () => reject(new Error("Channel socket could not connect.")), { once: true });
		});
		return {
			socket,
			nextBinary: (): Promise<Uint8Array> => {
				const frame = binary.shift();
				if (frame) return Promise.resolve(frame);
				if (socket.readyState !== WebSocket.OPEN) return Promise.reject(new Error("Tunnel socket is closed."));
				return new Promise((resolveFrame, reject) => { binaryReceiver = { resolve: resolveFrame, reject }; });
			},
			next: (): Promise<Frame> => {
				const frame = queued.shift();
				if (frame) return Promise.resolve(frame);
				if (socket.readyState !== WebSocket.OPEN) return Promise.reject(new Error("Channel socket is closed."));
				return new Promise((resolveFrame, reject) => { receiver = { resolve: resolveFrame, reject }; });
			},
			close: async () => {
				if (socket.readyState === WebSocket.CLOSED) return;
				await new Promise<void>((resolveClose) => {
					socket.addEventListener("close", () => resolveClose(), { once: true });
					socket.close();
				});
			},
		};
	}
	return { post, login, invoke, connect, closeSockets: () => { for (const socket of sockets) socket.close(); } };
}

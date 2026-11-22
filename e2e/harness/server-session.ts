import assert from "node:assert/strict";
import WebSocket from "ws";
import type { JsonObject } from "../../src/lib/types";
import { readinessTimeoutMs } from "./timeouts";

interface Session {
	resumeToken: string;
	sessionGraceSeconds: number;
	serverVersion: string;
	credentialStore: string;
}

type Frame = { type: string; channelId?: number; message?: { type: string }; error?: { kind: string }; resolvedPodName?: string; resolvedPodPort?: number };

export const channelMarker = (id: number) => `__KUBECOVE_CHANNEL__:${id}`;

function frameReader<T>(socket: WebSocket, description: string) {
	const queued: T[] = [];
	let pending: { resolve: (frame: T) => void; reject: (error: Error) => void } | undefined;
	socket.once("close", () => pending?.reject(new Error(`${description} socket closed before the expected frame.`)));
	return {
		push(frame: T) {
			if (!pending) { queued.push(frame); return; }
			const { resolve } = pending;
			pending = undefined;
			resolve(frame);
		},
		async next(): Promise<T> {
			const frame = queued.shift();
			if (frame !== undefined) return frame;
			if (socket.readyState !== WebSocket.OPEN) throw new Error(`${description} socket is closed.`);
			if (pending) throw new Error(`A ${description} frame read is already pending.`);
			return new Promise<T>((resolve, reject) => {
				const fail = (error: Error) => {
					clearTimeout(timer);
					pending = undefined;
					reject(error);
				};
				const timer = setTimeout(() => fail(new Error(`Timed out waiting for a ${description} frame.`)), readinessTimeoutMs);
				pending = { resolve: (frame) => { clearTimeout(timer); resolve(frame); }, reject: fail };
			});
		},
	};
}

export function serverClient(origin: string, token: string) {
	const sockets = new Set<WebSocket>();
	function postJson(path: string, body: string, headers: Record<string, string> = {}): Promise<Response> {
		return fetch(`${origin}${path}`, {
			method: "POST",
			headers: { "Content-Type": "application/json", Origin: origin, ...headers },
			body,
		});
	}
	const post = (path: string, body: JsonObject, headers?: Record<string, string>) => postJson(path, JSON.stringify(body), headers);

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

	function invoke<Args extends object>(session: Session, command: string, args: Args, cookie?: string): Promise<Response> {
		return postJson(`/api/v1/invoke/${command}`, JSON.stringify(args), {
			"X-Kubecove-Session": session.resumeToken,
			...(cookie ? { Cookie: cookie } : { Authorization: `Bearer ${token}` }),
		});
	}

	async function connect(session: Session, onFrame?: (frame: Frame) => void) {
		const url = new URL("/api/v1/channels", origin);
		url.protocol = "ws:";
		url.searchParams.set("resumeToken", session.resumeToken);
		const socket = new WebSocket(url, { headers: { Authorization: `Bearer ${token}` }, handshakeTimeout: readinessTimeoutMs });
		sockets.add(socket);
		const frames = frameReader<Frame>(socket, "Channel");
		const binary = frameReader<Uint8Array>(socket, "Tunnel");
		socket.on("message", (data, isBinary) => {
			let bytes: Buffer;
			if (data instanceof ArrayBuffer) bytes = Buffer.from(data);
			else if (Array.isArray(data)) bytes = Buffer.concat(data);
			else bytes = data;
			if (isBinary) {
				binary.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength));
				return;
			}
			const frame: Frame = JSON.parse(bytes.toString("utf8"));
			if (onFrame) { onFrame(frame); return; }
			frames.push(frame);
		});
		socket.addEventListener("close", () => {
			sockets.delete(socket);
		});
		await new Promise<void>((resolveOpen, reject) => {
			socket.addEventListener("open", () => resolveOpen(), { once: true });
			socket.on("error", () => reject(new Error("Channel socket could not connect.")));
		});
		return {
			socket,
			nextBinary: binary.next,
			next: frames.next,
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

import { Channel } from "@tauri-apps/api/core";
import { kubeconfigArg, sanitizeKubeconfigRequest } from "./tauri-args";
import { createMockChannel, shouldUseBrowserDevMocks, type TauriClient } from "./tauri-runtime";
import type {
	LiveSessionCleanupRequest,
	LiveSessionCleanupResult,
	PodExecSessionMessage,
	PodExecSessionRequest,
	PodExecSessionSummary,
	PodExecTerminalSize,
	PodLogStreamRequest,
	PortForwardRequest,
	PortForwardSessionSummary,
	StreamMessage,
	WatchResourceKey,
} from "./types";

export function createStreamChannel(
	onMessage: (message: StreamMessage) => void,
): Channel<StreamMessage> {
	if (shouldUseBrowserDevMocks()) return createMockChannel(onMessage);
	return new Channel<StreamMessage>(onMessage);
}

function closeChannel<T>(channel: Channel<T>): void {
	const disposableChannel = channel as unknown as {
		cleanupCallback?: () => void;
		unregister?: () => Promise<void>;
	};
	if (typeof disposableChannel.unregister === "function") {
		void disposableChannel.unregister();
		return;
	}
	disposableChannel.cleanupCallback?.();
}

export function closeStreamChannel(channel: Channel<StreamMessage>): void {
	closeChannel(channel);
}

export function createPodExecChannel(
	onMessage: (message: PodExecSessionMessage) => void,
): Channel<PodExecSessionMessage> {
	if (shouldUseBrowserDevMocks()) return createMockChannel(onMessage);
	return new Channel<PodExecSessionMessage>(onMessage);
}

export function closePodExecChannel(
	channel: Channel<PodExecSessionMessage>,
): void {
	closeChannel(channel);
}

export async function startResourceWatch(
	client: TauriClient,
	clusterContext: string,
	keys: WatchResourceKey[],
	channel: Channel<StreamMessage>,
	kubeconfigEnvVar?: string,
): Promise<string> {
	return client.invoke<string>("start_resource_watch", {
		clusterContext,
		keys,
		channel,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function startResourceEventWatch(
	client: TauriClient,
	clusterContext: string,
	kind: string,
	name: string,
	namespace: string | null | undefined,
	channel: Channel<StreamMessage>,
	kubeconfigEnvVar?: string,
): Promise<string> {
	return client.invoke<string>("start_resource_event_watch", {
		clusterContext,
		kind,
		name,
		namespace,
		channel,
		...kubeconfigArg(kubeconfigEnvVar),
	});
}

export async function startPodLogStream(
	client: TauriClient,
	request: PodLogStreamRequest,
	channel: Channel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_pod_log_stream", {
		request: sanitizeKubeconfigRequest(request),
		channel,
	});
}

export async function stopStream(
	client: TauriClient,
	streamId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_stream", { streamId });
}

export async function startPortForward(
	client: TauriClient,
	request: PortForwardRequest,
): Promise<PortForwardSessionSummary> {
	return client.invoke<PortForwardSessionSummary>("start_pod_port_forward", {
		request: sanitizeKubeconfigRequest(request),
	});
}

export async function startPodPortForward(
	client: TauriClient,
	request: PortForwardRequest,
): Promise<PortForwardSessionSummary> {
	return startPortForward(client, request);
}

export async function stopPodPortForward(
	client: TauriClient,
	sessionId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_port_forward", { sessionId });
}

export async function listPortForwards(
	client: TauriClient,
): Promise<PortForwardSessionSummary[]> {
	return client.invoke<PortForwardSessionSummary[]>("list_port_forwards");
}

export async function startPodExecSession(
	client: TauriClient,
	request: PodExecSessionRequest,
	channel: Channel<PodExecSessionMessage>,
): Promise<PodExecSessionSummary> {
	return client.invoke<PodExecSessionSummary>("start_pod_exec_session", {
		request: sanitizeKubeconfigRequest(request),
		channel,
	});
}

export async function writePodExecStdin(
	client: TauriClient,
	sessionId: string,
	data: string,
): Promise<boolean> {
	return client.invoke<boolean>("write_pod_exec_stdin", { sessionId, data });
}

export async function resizePodExecTerminal(
	client: TauriClient,
	sessionId: string,
	size: PodExecTerminalSize,
): Promise<boolean> {
	return client.invoke<boolean>("resize_pod_exec_terminal", { sessionId, size });
}

export async function stopPodExecSession(
	client: TauriClient,
	sessionId: string,
): Promise<boolean> {
	return client.invoke<boolean>("stop_pod_exec_session", { sessionId });
}

export async function listPodExecSessions(
	client: TauriClient,
): Promise<PodExecSessionSummary[]> {
	return client.invoke<PodExecSessionSummary[]>("list_pod_exec_sessions");
}

export async function stopLiveSessionsOutsideScope(
	client: TauriClient,
	request: LiveSessionCleanupRequest,
): Promise<LiveSessionCleanupResult> {
	return client.invoke<LiveSessionCleanupResult>(
		"stop_live_sessions_outside_scope",
		{ request },
	);
}

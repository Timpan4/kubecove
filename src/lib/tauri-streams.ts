import type { StreamChannel } from "./stream-channel";
import { kubeconfigArg, sanitizeKubeconfigRequest } from "./tauri-args";
import type { TauriClient } from "./tauri-runtime";
import type {
	AggregatedLogStreamRequest,
	LiveSessionCleanupRequest,
	LiveSessionCleanupResult,
	PodExecSessionMessage,
	PodExecSessionRequest,
	PodExecSessionSummary,
	PodExecTerminalSize,
	PodLogStreamRequest,
	PortForwardRequest,
	PortForwardSessionSummary,
	ResourceWatchStart,
	StreamMessage,
} from "./types";

export function createStreamChannel(
	onMessage: (message: StreamMessage) => void,
	client: TauriClient,
): StreamChannel<StreamMessage> {
	return openChannel(client, onMessage);
}

function openChannel<T>(client: TauriClient, onMessage: (message: T) => void): StreamChannel<T> {
	if (!client.createChannel) throw new Error("This client does not support streaming.");
	return client.createChannel(onMessage);
}

export function closeStreamChannel(channel: StreamChannel<StreamMessage>): void {
	channel.close();
}

export function createPodExecChannel(
	onMessage: (message: PodExecSessionMessage) => void,
	client: TauriClient,
): StreamChannel<PodExecSessionMessage> {
	return openChannel(client, onMessage);
}

export function closePodExecChannel(channel: StreamChannel<PodExecSessionMessage>): void {
	channel.close();
}

export async function startResourceWatch(
	client: TauriClient,
	clusterContext: string,
	keys: ResourceWatchStart[],
	channel: StreamChannel<StreamMessage>,
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
	channel: StreamChannel<StreamMessage>,
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
	channel: StreamChannel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_pod_log_stream", {
		request: sanitizeKubeconfigRequest({ ...request, batchLines: true }),
		channel,
	});
}

export async function startAggregatedLogStream(
	client: TauriClient,
	request: AggregatedLogStreamRequest,
	channel: StreamChannel<StreamMessage>,
): Promise<string> {
	return client.invoke<string>("start_aggregated_log_stream", {
		request: sanitizeKubeconfigRequest({ ...request, batchLines: true }),
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
	channel: StreamChannel<PodExecSessionMessage>,
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

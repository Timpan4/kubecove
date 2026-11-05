import type { Channel } from "@tauri-apps/api/core";
import { mockExecSessions, mockPortForwards, now, source } from "./tauri-dev-mock-data";
import type { LiveSessionCleanupRequest, PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary, PortForwardRequest, PortForwardSessionSummary } from "./types";

let serial = 0;
let workspaceOpened = false;
const forwards = new Map(mockPortForwards.map((s) => [s.id, s]));
const execs = new Map(mockExecSessions.map((s) => [s.id, s]));
const channels = new Map<string, Pick<Channel<PodExecSessionMessage>, "onmessage">>();
export type SessionScript = "reconnecting" | "failed" | "foreign-output";
export function forwardReplayState(script: Exclude<SessionScript, "foreign-output">): Pick<PortForwardSessionSummary, "status" | "lastError"> {
    return script === "failed" ? { status: "error", lastError: "Controlled transport failure" } : { status: "reconnecting" };
}
export function replaySessions(script: SessionScript) {
    if (script === "foreign-output") {
        for (const channel of channels.values()) channel.onmessage({ type: "output", sessionId: "foreign-session", stream: "stdout", data: "FOREIGN SESSION OUTPUT" });
    } else {
        for (const [id, session] of forwards) forwards.set(id, { ...session, ...forwardReplayState(script) });
    }
}

export function startMockForward(request: PortForwardRequest): PortForwardSessionSummary {
    const kind = request.targetKind ?? "Pod";
    const name = request.targetName ?? request.podName ?? "";
    if (!request.namespace || !name || !["Pod", "Service"].includes(kind)) throw new Error("Exact namespaced Pod or selector-backed Service required");
    // This controlled Service has selector app=payments-api and targetPort 8080.
    // No arbitrary Service is treated as selector-backed.
    if (kind === "Service" && !(name === "payments-api" && request.namespace === "payments" && request.remotePort === 80)) throw new Error("No ready matching Pod for this Service port");
    const pod = kind === "Service" ? "payments-api-7d9c9b7f8d-k2r9p" : name;
    const local = request.localPort ?? request.remotePort;
    if (Array.from(forwards.values()).some((s) => s.localPort === local)) throw new Error("Local port is already in use");
    const session: PortForwardSessionSummary = { id: `controlled-forward-${++serial}`, clusterContext: request.clusterContext, kubeconfigSourceKey: source.sourceKey, namespace: request.namespace, targetKind: kind, targetName: name, podName: pod, remotePort: request.remotePort, resolvedPodName: pod, resolvedPodPort: kind === "Service" ? 8080 : request.remotePort, localPort: local, localAddress: "127.0.0.1", localUrl: `http://127.0.0.1:${local}`, status: "listening", startedAt: now };
    forwards.set(session.id, session);
    return session;
}
export function startMockExec(request: PodExecSessionRequest, channel: Pick<Channel<PodExecSessionMessage>, "onmessage">): PodExecSessionSummary {
    const target = `${request.clusterContext}/${request.namespace}/Pod/${request.podName}/container/${request.container || "<default>"}`;
    if (!request.confirmation.acknowledged || request.confirmation.command !== JSON.stringify(request.command) || request.confirmation.target !== target) throw new Error("Exec confirmation required");
    if (!request.namespace || !request.podName || request.command.length === 0) throw new Error("Exact Pod and argv required");
    const session: PodExecSessionSummary = { id: `controlled-exec-${++serial}`, clusterContext: request.clusterContext, kubeconfigSourceKey: source.sourceKey, namespace: request.namespace, podName: request.podName, container: request.container, command: request.command, stdin: request.stdin, tty: request.tty, terminalCols: request.terminalSize.cols, terminalRows: request.terminalSize.rows, status: "running", startedAt: now };
    execs.set(session.id, session);
    channels.set(session.id, channel);
    channel.onmessage({ type: "started", sessionId: session.id, summary: session });
    channel.onmessage({ type: "output", sessionId: session.id, stream: "stdout", data: `Controlled exec ${request.container ?? "default"}\r\n$ ` });
    return session;
}
export function mockExecInput(id: string, data: string): boolean {
    const channel = channels.get(id);
    if (!channel) throw new Error("Exec session is closed");
    channel.onmessage({ type: "output", sessionId: id, stream: "stdout", data });
    return true;
}
export function resizeMockExec(id: string, size: { cols: number; rows: number }): boolean {
    const session = execs.get(id);
    if (!session) return false;
    execs.set(id, { ...session, terminalCols: size.cols, terminalRows: size.rows });
    return true;
}
export function stopMockExec(id: string): boolean {
    channels.get(id)?.onmessage({ type: "stopped", sessionId: id });
    channels.delete(id);
    return execs.delete(id);
}
export const listMockForwards = () => [...forwards.values()];
export const listMockExecs = () => [...execs.values()];
export const stopMockForward = (id: string) => forwards.delete(id);
export function cleanupMockSessions(request: LiveSessionCleanupRequest) {
    // Seeded sessions describe an already open mock workspace. The launcher's
    // initial empty scope must not erase that fixture before its first open.
    if (!workspaceOpened && request.allowedClusterContexts.length === 0) return { stoppedPortForwardIds: [], stoppedPodExecIds: [], stoppedPortForwards: 0, stoppedPodExecSessions: 0 };
    workspaceOpened = true;
    const outside = (s: PortForwardSessionSummary | PodExecSessionSummary) => !request.allowedClusterContexts.includes(s.clusterContext) || s.kubeconfigSourceKey !== request.kubeconfigSourceKey;
    const stoppedPortForwardIds = [...forwards.values()].filter(outside).map((s) => s.id);
    const stoppedPodExecIds = [...execs.values()].filter(outside).map((s) => s.id);
    stoppedPortForwardIds.forEach(stopMockForward);
    stoppedPodExecIds.forEach(stopMockExec);
    return { stoppedPortForwardIds, stoppedPodExecIds, stoppedPortForwards: stoppedPortForwardIds.length, stoppedPodExecSessions: stoppedPodExecIds.length };
}

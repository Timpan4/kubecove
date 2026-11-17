import { buildPodExecRequest } from "../../../src/features/live-sessions/podExecHelpers";
import type { TauriClient } from "../../../src/lib/tauri";
import { execEndMessage, forwardReplayState } from "../../../src/lib/tauri-dev-mock-session-controls";
import type { PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary, PortForwardRequest, PortForwardSessionSummary, ResourceSummary } from "../../../src/lib/types";

export async function exportSessionControls(client: TauriClient, resources: ResourceSummary[]) {
    const forwardRequests: PortForwardRequest[] = [
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Pod", targetName: "payments-api-7d9c9b7f8d-k2r9p", remotePort: 9090, localPort: 19090 },
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Pod", targetName: "payments-api-7d9c9b7f8d-k2r9p", remotePort: 9090, localPort: 19091 },
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Service", targetName: "payments-api", remotePort: 80, localPort: 18081 },
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Service", targetName: "payments-api", remotePort: 80, localPort: 18082 },
    ];
    const forwards = [];
    for (const request of forwardRequests) {
        const summary = await client.invoke<PortForwardSessionSummary>("start_pod_port_forward", { request });
        forwards.push({ request, summary });
        await client.invoke("stop_port_forward", { sessionId: summary.id });
    }
    const execPod = (context: string, name: string) => {
        const pod = resources.find((row) => row.cluster === context && row.kind === "Pod" && row.name === name);
        if (!pod) throw new Error(`Missing exec Pod fixture ${context}/${name}`);
        return pod;
    };
    const pod = execPod("mock-dev", "payments-api-7d9c9b7f8d-k2r9p");
    const execs: { request: PodExecSessionRequest; summary: PodExecSessionSummary; messages: PodExecSessionMessage[] }[] = [];
    const exportExec = async (pod: ResourceSummary, container: string, command: string[]) => {
        const built = buildPodExecRequest(pod, { container, command, cols: 100, rows: 32 });
        if (!built.ok) throw new Error(built.message);
        const messages: PodExecSessionMessage[] = [];
        if (!client.createChannel) throw new Error("Browser fixture client requires controlled channels");
        const channel = client.createChannel((message: PodExecSessionMessage) => messages.push(message));
        const summary = await client.invoke<PodExecSessionSummary>("start_pod_exec_session", { request: built.request, channel });
        execs.push({ request: built.request, summary, messages: [...messages] });
        await client.invoke("stop_pod_exec_session", { sessionId: summary.id });
    };
    for (const container of ["app", "sidecar"]) {
        for (const command of [["/bin/sh"], ["/bin/bash"], ["/usr/bin/env", "printenv"]]) {
            await exportExec(pod, container, command);
        }
    }
    await exportExec(execPod("admin@solid-k8s", "metrics-gateway-9f6d4f7c9-7j2p4"), "app", ["/bin/sh"]);
    // The native mock replaces the empty session id with each running session's id.
    const execEnds = Object.fromEntries((["exec-exited", "exec-failed", "exec-stopped"] as const).map((script) => [script, execEndMessage(script, "")]));
    return { forwards, execs, replay: { failed: forwardReplayState("failed"), reconnecting: forwardReplayState("reconnecting") }, execEnds };
}

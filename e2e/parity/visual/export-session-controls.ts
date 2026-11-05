import { buildPodExecRequest } from "../../../src/features/live-sessions/podExecHelpers";
import type { TauriClient } from "../../../src/lib/tauri";
import { forwardReplayState } from "../../../src/lib/tauri-dev-mock-session-controls";
import type { PodExecSessionMessage, PodExecSessionRequest, PodExecSessionSummary, PortForwardRequest, PortForwardSessionSummary, ResourceSummary } from "../../../src/lib/types";

export async function exportSessionControls(client: TauriClient, resources: ResourceSummary[]) {
    const forwardRequests: PortForwardRequest[] = [
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Pod", targetName: "payments-api-7d9c9b7f8d-k2r9p", remotePort: 9090, localPort: 19090 },
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Service", targetName: "payments-api", remotePort: 80, localPort: 18081 },
        { clusterContext: "mock-dev", namespace: "payments", targetKind: "Service", targetName: "payments-api", remotePort: 80, localPort: 18082 },
    ];
    const forwards = [];
    for (const request of forwardRequests) {
        const summary = await client.invoke<PortForwardSessionSummary>("start_pod_port_forward", { request });
        forwards.push({ request, summary });
        await client.invoke("stop_port_forward", { sessionId: summary.id });
    }
    const pod = resources.find((r) => r.kind === "Pod" && r.name === "payments-api-7d9c9b7f8d-k2r9p");
    if (!pod) throw new Error("Missing exec Pod fixture");
    const execs: { request: PodExecSessionRequest; summary: PodExecSessionSummary; messages: PodExecSessionMessage[] }[] = [];
    for (const container of ["app", "sidecar"]) {
        for (const command of [["/bin/sh"], ["/bin/bash"], ["/usr/bin/env", "printenv"]]) {
            const built = buildPodExecRequest(pod, { container, command, cols: 100, rows: 32 });
            if (!built.ok) throw new Error(built.message);
            const messages: PodExecSessionMessage[] = [];
            if (!client.createChannel) throw new Error("Browser fixture client requires controlled channels");
            const channel = client.createChannel((m: PodExecSessionMessage) => messages.push(m));
            const summary = await client.invoke<PodExecSessionSummary>("start_pod_exec_session", { request: built.request, channel });
            execs.push({ request: built.request, summary, messages: [...messages] });
            await client.invoke("stop_pod_exec_session", { sessionId: summary.id });
        }
    }
    return { forwards, execs, replay: { failed: forwardReplayState("failed"), reconnecting: forwardReplayState("reconnecting") } };
}

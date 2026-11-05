import { applicationSpecFromYaml } from "../../../src/features/gitops/argo-application-yaml";
import type { TauriClient } from "../../../src/lib/tauri";
import type { ArgoApplicationInspector, ArgoApplicationRef, ArgoConnectionStatus, ArgoOperationRequest, ResourceDetailsFull } from "../../../src/lib/types";

/** Exact native requests from the same mock command implementation as web. */
export async function exportArgo(client: TauriClient, applications: { details: ResourceDetailsFull }[]) {
    const url = "https://argocd.example.test";
    const profile = { id: `argo:native-session:mock-dev:external:${url}`, url, endpoint: { kind: "externalHttps" as const, url }, clusterContext: "mock-dev", workspaceId: "native-session", transport: "connected", rememberCredential: false };
    const connected = await client.invoke<ArgoConnectionStatus>("connect_argo_server", { id: profile.id, serverUrl: url, endpoint: profile.endpoint, clusterContext: "mock-dev", workspaceId: "native-session", insecureTls: false, rememberCredential: false });
    const connect = { profile: { ...profile, kubeconfigSourceKey: connected.profile?.kubeconfigSourceKey }, token: "parity-fixture-token" };
    const inspections = [];
    const operations = [];
    for (const { details } of applications) {
        const application: ArgoApplicationRef = { name: details.summary.name, namespace: details.summary.namespace ?? null, project: null, resourceVersion: null, uid: null, apiVersion: details.summary.apiVersion ?? null, context: "mock-dev", workspaceId: "native-session" };
        for (const connectionId of [null, profile.id]) {
            const transport = connectionId ? "connected" : "kubernetes";
            const result = await client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", { clusterContext: "mock-dev", application, transport, connectionId });
            inspections.push({ request: { application, connectionId }, result });
            if (application.name !== "tenant-ledger") continue;
            for (const action of ["refresh", "sync", "rollback"] as const) {
                const request: ArgoOperationRequest = { connectionId, transport, application: result.application, action, revision: null, resources: [], prune: action === "sync" ? automatedPrune(applicationSpecFromYaml(details.yaml).syncPolicy) : null, dryRun: action === "sync" ? false : null, force: action === "sync" ? false : null, historyId: action === "rollback" ? 17 : null, resourceAction: null, resourceActionParameters: null, resourceVersion: "42", clusterContext: "mock-dev", kubeconfigEnvVar: null, syncPayload: null };
                const review = await client.invoke("preflight_argo_operation", { request });
                operations.push({ request, review });
            }
        }
    }
    const confirmation = { sessionId: "mock-session", confirmation: "mock-session" };
    const result = await client.invoke("run_argo_operation", { confirmation });
    await client.invoke("disconnect_argo_server", { id: profile.id });
    return { connect, connected, inspections, operations, confirmation, result };
}
function automatedPrune(policy: unknown): boolean {
    if (!policy || typeof policy !== "object" || !("automated" in policy)) return false;
    const automated = policy.automated;
    return !!automated && typeof automated === "object" && "prune" in automated && automated.prune === true;
}

import { applicationSpecFromYaml } from "../../../src/features/gitops/argo-application-yaml";
import { argoProfileId } from "../../../src/features/gitops/argo-connection";
import type { TauriClient } from "../../../src/lib/tauri";
import type { ArgoApplicationInspector, ArgoApplicationRef, ArgoConnectionStatus, ArgoOperationRequest, ResourceDetailsFull } from "../../../src/lib/types";

/** Exact native requests from the same mock command implementation as web. */
export async function exportArgo(client: TauriClient, applications: { details: ResourceDetailsFull }[]) {
    const url = "https://argocd.example.test";
    const source = await client.invoke<{ sourceKey: string }>("get_kubeconfig_sources");
    const scope = { workspaceId: "native-session", clusterContext: "mock-dev", kubeconfigSourceKey: source.sourceKey };
    const profile = { id: argoProfileId(scope, { kind: "externalHttps", url }), url, endpoint: { kind: "externalHttps" as const, url }, clusterContext: "mock-dev", workspaceId: "native-session", transport: "connected", rememberCredential: false };
    const connected = await client.invoke<ArgoConnectionStatus>("connect_argo_server", { id: profile.id, serverUrl: url, endpoint: profile.endpoint, clusterContext: "mock-dev", workspaceId: "native-session", insecureTls: false, rememberCredential: false });
    const connect = { profile: { ...profile, kubeconfigSourceKey: connected.profile?.kubeconfigSourceKey }, token: "parity-fixture-token", username: null, password: null, insecureTls: false, customCaPem: null, confirmedTarget: null };
    const discovery = await client.invoke<import("../../../src/lib/types").ArgoServerCapability[]>("discover_argo_servers", { clusterContext: "mock-dev" });
    const tunnelEndpoint = discovery[0].endpoint;
    if (!tunnelEndpoint) throw new Error("Missing tunnel fixture");
    const tunnelId = argoProfileId(scope, tunnelEndpoint);
    const target = { context: "mock-dev", source: connected.profile?.kubeconfigSourceKey, endpoint: tunnelEndpoint };
    const preview = await client.invoke<{ namespace: string; serviceName: string; podName: string; certificateSha256: string }>("preview_argo_tunnel_target", { clusterContext: "mock-dev", endpoint: tunnelEndpoint });
    const variants = [
        connect,
        { ...connect, profile: { ...connect.profile, rememberCredential: true } },
        { ...connect, token: null, profile: { ...connect.profile, rememberCredential: true } },
        { ...connect, token: null, username: "fixture-user", password: "fixture-password", insecureTls: true, customCaPem: [...new TextEncoder().encode("parity fixture CA")] },
        { ...connect, profile: { ...connect.profile, id: tunnelId, url: "https://argocd-server.argocd.svc", endpoint: tunnelEndpoint }, confirmedTarget: preview },
    ];
    const connections = variants.map(request => ({ request, result: { ...connected, profile: request.profile } }));
    const inspections = [];
    const operations = [];
    for (const { details } of applications) {
        const application: ArgoApplicationRef = { name: details.summary.name, namespace: details.summary.namespace ?? null, project: null, resourceVersion: null, uid: null, apiVersion: details.summary.apiVersion ?? null, context: "mock-dev", workspaceId: "native-session" };
        for (const connectionId of [null, profile.id, tunnelId]) {
            const transport = connectionId ? "connected" : "kubernetes";
            const result = await client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", { clusterContext: "mock-dev", application, transport, connectionId });
            inspections.push({ request: { application, connectionId, source: connected.profile?.kubeconfigSourceKey }, result });
            inspections.push({ request: { application: result.application, connectionId, source: connected.profile?.kubeconfigSourceKey }, result });
            if (application.name !== "tenant-ledger") continue;
            for (const action of ["refresh", "sync", "rollback"] as const) {
                const request: ArgoOperationRequest = { connectionId, transport, application: result.application, action, revision: null, resources: [], prune: action === "sync" ? automatedPrune(applicationSpecFromYaml(details.yaml).syncPolicy) : null, dryRun: action === "sync" ? false : null, force: action === "sync" ? false : null, historyId: action === "rollback" ? 17 : null, resourceAction: null, resourceActionParameters: null, resourceVersion: "42", clusterContext: "mock-dev", kubeconfigEnvVar: connected.profile?.kubeconfigSourceKey ?? null, syncPayload: null };
                const review = await client.invoke("preflight_argo_operation", { request });
                operations.push({ request, review });
                if (action === "rollback") {
                    const selected = { ...request, historyId: 18 };
                    operations.push({ request: selected, review: await client.invoke("preflight_argo_operation", { request: selected }) });
                }
                if (action === "sync") {
                    const advanced = { ...request, revision: "3d91a7b", prune: false, dryRun: true, force: true };
                    operations.push({ request: advanced, review: await client.invoke("preflight_argo_operation", { request: advanced }) });
                }
            }
        }
    }
    const confirmation = { sessionId: "mock-session", confirmation: "mock-session" };
    const result = await client.invoke("run_argo_operation", { confirmation });
    await client.invoke("disconnect_argo_server", { id: profile.id });
    return { connect, connected, connections, discovery, target, preview, inspections, operations, confirmation, result };
}
function automatedPrune(policy: unknown): boolean {
    if (!policy || typeof policy !== "object" || !("automated" in policy)) return false;
    const automated = policy.automated;
    return !!automated && typeof automated === "object" && "prune" in automated && automated.prune === true;
}

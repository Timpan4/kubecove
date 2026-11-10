import { applicationSpecFromYaml } from "../../../src/features/gitops/argo-application-yaml";
import { argoProfileId } from "../../../src/features/gitops/argo-connection";
import type { TauriClient } from "../../../src/lib/tauri";
import { configureArgoMock } from "../../../src/lib/tauri-dev-mock-argo";
import type { ArgoApplicationInspector, ArgoApplicationRef, ArgoConnectionStatus, ArgoOperationRequest, JsonObject, JsonValue, ResourceDetailsFull } from "../../../src/lib/types";

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
    const target = { context: "mock-dev", source: connected.profile?.kubeconfigSourceKey, endpoint: tunnelEndpoint };
    const preview = await client.invoke<{ namespace: string; serviceName: string; podName: string; certificateSha256: string }>("preview_argo_tunnel_target", { clusterContext: "mock-dev", endpoint: tunnelEndpoint });
    // A reconfirm previews the saved profile endpoint, which the connect path normalizes (TLS name filled in).
    const sessionTunnelId = argoProfileId(scope, tunnelEndpoint);
    const saved = await client.invoke<ArgoConnectionStatus>("connect_argo_server", { id: sessionTunnelId, serverUrl: "https://argocd-server.argocd.svc", endpoint: tunnelEndpoint, clusterContext: "mock-dev", workspaceId: "native-session", kubeconfigEnvVar: connected.profile?.kubeconfigSourceKey, insecureTls: false, rememberCredential: true });
    const savedEndpoint = saved.profile?.endpoint;
    await client.invoke("disconnect_argo_server", { id: sessionTunnelId });
    if (savedEndpoint?.kind !== "serviceTunnel") throw new Error("Missing saved tunnel endpoint");
    const reconfirmPreview = await client.invoke<{ namespace: string; serviceName: string; podName: string; certificateSha256: string }>("preview_argo_tunnel_target", { clusterContext: "mock-dev", endpoint: savedEndpoint });
    const previews = [{ request: target, result: preview }, { request: { ...target, endpoint: savedEndpoint }, result: reconfirmPreview }];
    // The native client keys Argo connections by its workspace id: "native-session" before a
    // workspace is saved, and the saved record's id ("1") once the startup workspace is saved.
    const connectionIds = (workspaceId: string) => {
        const requestScope = { ...scope, workspaceId };
        return { profile: argoProfileId(requestScope, { kind: "externalHttps", url }), tunnel: argoProfileId(requestScope, tunnelEndpoint) };
    };
    const connectVariants = (workspaceId: string) => {
        const ids = connectionIds(workspaceId);
        const tunnel = ids.tunnel;
        const connect = { profile: { ...profile, id: ids.profile, workspaceId, kubeconfigSourceKey: connected.profile?.kubeconfigSourceKey }, token: "parity-fixture-token", username: null, password: null, insecureTls: false, customCaPem: null, confirmedTarget: null };
        return [
            connect,
            // Confirm target again after a failed reconnect sends the saved endpoint, with its TLS name.
            { ...connect, token: null, profile: { ...connect.profile, id: tunnel, url: "https://argocd-server.argocd.svc", endpoint: savedEndpoint, rememberCredential: true }, confirmedTarget: preview },
            // An automatic reconnect after a restart sends the saved endpoint and the saved target.
            { ...connect, token: null, profile: { ...connect.profile, id: tunnel, url: "https://argocd-server.argocd.svc", endpoint: savedEndpoint, rememberCredential: true }, confirmedTarget: { namespace: preview.namespace, serviceName: preview.serviceName, podName: null, certificateSha256: null } },
            { ...connect, profile: { ...connect.profile, rememberCredential: true } },
            { ...connect, token: null, profile: { ...connect.profile, rememberCredential: true } },
            { ...connect, token: null, username: "fixture-user", password: "fixture-password", insecureTls: true, customCaPem: [...new TextEncoder().encode("parity fixture CA")] },
            { ...connect, profile: { ...connect.profile, id: tunnel, url: "https://argocd-server.argocd.svc", endpoint: tunnelEndpoint }, confirmedTarget: preview },
            ...["initial", "remembered", "confirmed"].map(mode => ({ ...connect, token: mode === "initial" ? connect.token : null, profile: { ...connect.profile, id: tunnel, url: "https://argocd-server.argocd.svc", endpoint: tunnelEndpoint, rememberCredential: true }, confirmedTarget: mode === "remembered" ? { namespace: preview.namespace, serviceName: preview.serviceName, podName: null, certificateSha256: null } : preview })),
        ];
    };
    const variants = [...connectVariants("native-session"), ...connectVariants("1")];
    const connections = variants.map(request => ({ request, result: { ...connected, profile: request.profile } }));
    const inspections = [];
    const comparisons = [];
    const operations = [];
    for (const { details } of applications) {
        for (const workspaceId of ["native-session", "1"]) {
            const application: ArgoApplicationRef = { name: details.summary.name, namespace: details.summary.namespace ?? null, project: null, resourceVersion: null, uid: null, apiVersion: details.summary.apiVersion ?? null, context: "mock-dev", workspaceId };
            const ids = connectionIds(workspaceId);
            for (const connectionId of [null, ids.profile, ids.tunnel]) {
                const transport = connectionId ? "connected" : "kubernetes";
                const result = await client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", { clusterContext: "mock-dev", application, transport, connectionId });
                configureArgoMock("full-diff");
                const full = await client.invoke<ArgoApplicationInspector>("get_argo_application_inspector", { clusterContext: "mock-dev", application, transport, connectionId });
                configureArgoMock("base");
                inspections.push({ request: { application, connectionId, source: connected.profile?.kubeconfigSourceKey }, result, full });
                inspections.push({ request: { application: result.application, connectionId, source: connected.profile?.kubeconfigSourceKey }, result, full });
                if (connectionId) for (const resource of result.resources) {
                    const selected = { ...resource, targetState: null, liveState: null };
                    comparisons.push({ request: { inspection: { application: result.application, connectionId, source: connected.profile?.kubeconfigSourceKey }, resource: selected }, result: await client.invoke("get_argo_resource_comparison", { clusterContext: "mock-dev", application: result.application, connectionId, transport: "connected", resource: selected }) });
                }
                if (application.name !== "tenant-ledger") continue;
                for (const action of ["refresh", "hardRefresh", "sync", "rollback"] as const) {
                    const request: ArgoOperationRequest = { connectionId, transport, application: result.application, action, revision: null, resources: [], prune: action === "sync" ? automatedPrune(applicationSpecFromYaml(details.yaml).syncPolicy) : null, dryRun: action === "sync" ? false : null, force: action === "sync" ? false : null, historyId: action === "rollback" ? 17 : null, resourceAction: null, resourceActionParameters: null, resourceVersion: "42", clusterContext: "mock-dev", kubeconfigEnvVar: connected.profile?.kubeconfigSourceKey ?? null, syncPayload: null };
                    const review = await client.invoke("preflight_argo_operation", { request });
                    operations.push({ request, review });
                    if (action === "rollback") {
                        const selected = { ...request, historyId: 18 };
                        operations.push({ request: selected, review: await client.invoke("preflight_argo_operation", { request: selected }) });
                    }
                    if (action === "sync") {
                        // Prune starts checked from the automated Application; configureArgoSync unchecks it and checks the other two.
                        const advanced = { ...request, revision: "3d91a7b", prune: false, dryRun: true, force: true };
                        operations.push({ request: advanced, review: await client.invoke("preflight_argo_operation", { request: advanced }) });
                    }
                }
            }
        }
    }
    const confirmation = { sessionId: "mock-session", confirmation: "mock-session" };
    const result = await client.invoke("run_argo_operation", { confirmation });
    await client.invoke("disconnect_argo_server", { id: profile.id });
    return { connect, connected, connections, discovery, target, previews, inspections, comparisons, operations, confirmation, result };
}
function isJsonRecord<Value>(value: Value): value is Value & JsonObject {
    return value !== null && !Array.isArray(value) && Object(value) === value;
}

function automatedPrune(policy: JsonValue | undefined): boolean {
    return isJsonRecord(policy) && isJsonRecord(policy.automated) && policy.automated.prune === true;
}

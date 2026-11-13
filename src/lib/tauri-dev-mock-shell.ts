import overviewPatches from "./tauri-dev-mock-overview.json";
import { PAGE_SIZE } from "@/features/resources/constants";
import type { ResourceSummary } from "./types";
export type ShellReadPhase = "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "namespaces-credential-failed" | "namespaces-empty" | "resources-failed" | "primary-context-missing" | "kind-missing" | "payments-api-removed" | "payments-deployments-forbidden" | "payments-ingress-version-changed" | "sources-failed" | "overview-owner-chain" | "overview-many";
function initialPhase(): ShellReadPhase {
    const value = globalThis.sessionStorage?.getItem("parity-shell-read");
    switch (value) {
        case "contexts-failed": case "contexts-empty": case "namespaces-empty": case "namespaces-failed": case "namespaces-credential-failed": case "resources-failed":
        case "primary-context-missing": case "kind-missing": case "payments-api-removed": case "payments-deployments-forbidden": case "payments-ingress-version-changed": case "sources-failed": case "overview-owner-chain": case "overview-many": return value;
        default: return "ready";
    }
}
let phase: ShellReadPhase = initialPhase();

export function configureShellRead(next: ShellReadPhase): void {
    phase = next;
    globalThis.sessionStorage?.setItem("parity-shell-read", next);
}

export function shellRead<Item extends object>(read: "contexts" | "namespaces" | "resources" | "kinds", items: Item[]): Item[] {
    if (phase === `${read}-failed`) throw new Error(`Scripted ${read} read failure`);
    // Typed like the timing mock's failures, so the friendly error classifies by kind.
    if (read === "namespaces" && phase === "namespaces-credential-failed") throw Object.assign(new Error("Scripted read: credentialUnavailable"), { kind: "credentialUnavailable" });
    if ((read === "namespaces" && phase === "namespaces-empty") || (read === "contexts" && phase === "contexts-empty")) return [];
    // The saved primary context of the parity workspaces is mock-dev.
    if (read === "contexts" && phase === "primary-context-missing") return items.filter((item) => !("name" in item) || item.name !== "mock-dev");
    if (read === "kinds" && phase === "kind-missing") return [];
    return items;
}

/** Fails kubeconfig source reads while the `sources-failed` phase is set. */
export function shellKubeconfigSourcesRead<Sources>(sources: Sources): Sources {
    if (phase === "sources-failed") throw new Error("Scripted kubeconfig sources read failure");
    return sources;
}

export interface ScopeRequestSplit<Request> {
    completed: Request[];
    forbidden: Request[];
}

/** Splits scope requests into lists that complete and the Deployment lists the phase refuses. */
export function shellScopeRequests<Request extends { kind?: string }>(requests: Request[]): ScopeRequestSplit<Request> {
    const refused = (request: Request) => phase === "payments-deployments-forbidden" && request.kind === "Deployment";
    return { completed: requests.filter((request) => !refused(request)), forbidden: requests.filter(refused) };
}

/** Applies the scripted cluster changes after a resource was pinned: a deleted Deployment, or an Ingress served at another API version. */
export function shellWithoutRemoved(rows: ResourceSummary[]): ResourceSummary[] {
    if (phase === "overview-many") {
        const template = rows.find(row => row.name === "operations-crashloop-7f468cd8b8-lab01");
        return template ? Array.from({ length: PAGE_SIZE + 1 }, (_, index) => ({ ...template, name: `overview-finding-${String(index).padStart(3, "0")}`, ownerRef: undefined })) : [];
    }
    if (phase === "overview-owner-chain") return rows.map((row): ResourceSummary => {
        const patch = overviewPatches.find(patch => patch.kind === row.kind && patch.namespace === row.namespace && patch.name === row.name);
        return patch ? { ...row, ...patch, healthAssessment: { state: "degraded", completeness: "complete", winningSources: ["kubernetes"], reasons: ["Controlled owner-chain failure"], evidence: [] } } : row;
    });
    const payments = (row: ResourceSummary, kind: string) => row.kind === kind && row.namespace === "payments" && row.name === "payments-api";
    if (phase === "payments-ingress-version-changed") return rows.map((row) => (payments(row, "Ingress") ? { ...row, apiVersion: "networking.k8s.io/v1beta1" } : row));
    if (phase !== "payments-api-removed") return rows;
    return rows.filter((row) => !payments(row, "Deployment"));
}

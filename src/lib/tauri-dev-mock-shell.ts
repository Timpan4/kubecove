export type ShellReadPhase = "ready" | "contexts-failed" | "contexts-empty" | "namespaces-failed" | "namespaces-credential-failed" | "namespaces-empty" | "resources-failed" | "primary-context-missing" | "kind-missing" | "payments-api-removed" | "sources-failed";
function initialPhase(): ShellReadPhase {
    const value = globalThis.sessionStorage?.getItem("parity-shell-read");
    switch (value) {
        case "contexts-failed": case "contexts-empty": case "namespaces-empty": case "namespaces-failed": case "namespaces-credential-failed": case "resources-failed":
        case "primary-context-missing": case "kind-missing": case "payments-api-removed": case "sources-failed": return value;
        default: return "ready";
    }
}
let phase: ShellReadPhase = initialPhase();

export function configureShellRead(next: ShellReadPhase): void {
    phase = next;
    globalThis.sessionStorage?.setItem("parity-shell-read", next);
}

export function shellRead<Item>(read: "contexts" | "namespaces" | "resources" | "kinds", items: Item[]): Item[] {
    if (phase === `${read}-failed`) throw new Error(`Scripted ${read} read failure`);
    // Typed like the timing mock's failures, so the friendly error classifies by kind.
    if (read === "namespaces" && phase === "namespaces-credential-failed") throw Object.assign(new Error("Scripted read: credentialUnavailable"), { kind: "credentialUnavailable" });
    if ((read === "namespaces" && phase === "namespaces-empty") || (read === "contexts" && phase === "contexts-empty")) return [];
    // The saved primary context of the parity workspaces is mock-dev.
    if (read === "contexts" && phase === "primary-context-missing") return items.filter((item) => (item as { name?: string }).name !== "mock-dev");
    if (read === "kinds" && phase === "kind-missing") return [];
    return items;
}

/** Fails kubeconfig source reads while the `sources-failed` phase is set. */
export function shellKubeconfigSourcesRead<Sources>(sources: Sources): Sources {
    if (phase === "sources-failed") throw new Error("Scripted kubeconfig sources read failure");
    return sources;
}

/** Removes the scripted Deployment from scope reads, as if it were deleted from the cluster. */
export function shellWithoutRemoved<Row extends { kind: string; namespace?: string | null; name: string }>(rows: Row[]): Row[] {
    if (phase !== "payments-api-removed") return rows;
    return rows.filter((row) => !(row.kind === "Deployment" && row.namespace === "payments" && row.name === "payments-api"));
}

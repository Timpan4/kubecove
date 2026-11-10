import { emitMockKindChange } from "./tauri-dev-mock-live";
import type { JsonObject } from "./types";

export const PAGE_COMMANDS = ["list_argocd_applications", "list_argocd_appsets", "list_argocd_appprojects", "list_flux_resources", "list_helm_releases", "list_rbac_inspection", "list_incident_cockpit"] as const;
export type PageReadPhase = "base" | "updated" | "failed";
let phase: PageReadPhase = "base";

export function replayPageReads(next: PageReadPhase) {
    phase = next;
    emitMockKindChange("ApplicationSet");
    emitMockKindChange("Kustomization");
}

function record<Value>(value: Value | JsonObject): value is JsonObject {
    return value !== null && !Array.isArray(value) && Object(value) === value;
}

/** Adds a distinct object while keeping existing selections valid. */
function appendRow<Row>(rows: Row[], nested: boolean) {
    const first = rows[0];
    if (!record(first)) throw new Error("Page fixture needs a row");
    if (nested) {
        if (!record(first.resource)) throw new Error("Incident fixture needs a resource");
        return [...rows, { ...first, resource: { ...first.resource, name: `live-${first.resource.name}` } }];
    }
    return [...rows, { ...first, name: `live-${first.name}` }];
}

export function pageReadResult<Result>(command: string, value: Result): Result {
    if (!PAGE_COMMANDS.some((candidate) => candidate === command) || phase === "base") return value;
    if (phase === "failed") throw new Error("Scripted page refresh failure");
    if (!record(value)) return value;
    // ApplicationSet and Flux lists are versioned lists, which keep their rows in `items`.
    const field = command === "list_argocd_appsets" || command === "list_flux_resources" ? "items" : command === "list_helm_releases" ? "releases" : command === "list_rbac_inspection" ? "serviceAccounts" : command === "list_incident_cockpit" ? "items" : null;
    if (!field) return value;
    const rows = value[field];
    if (!Array.isArray(rows)) throw new Error(`Missing page fixture ${field}`);
    return { ...value, [field]: appendRow(rows, command === "list_incident_cockpit") };
}

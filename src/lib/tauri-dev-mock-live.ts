import type { Channel } from "@tauri-apps/api/core";
import type { ResourceEventSummary, ResourceSummary, StreamMessage, WatchResourceKey } from "./types";

export const LIVE_EVENTS: ResourceEventSummary[] = [
    { eventType: "Normal", reason: "LiveOlder", message: "Older scripted event", count: 1, lastSeen: "2m", lastSeenAt: "2026-06-29T10:01:00Z", source: "parity", namespace: "payments" },
    { eventType: "Normal", reason: "LiveNewest", message: "Newest scripted event", count: 1, lastSeen: "1m", lastSeenAt: "2026-06-29T10:02:00Z", source: "parity", namespace: "payments" },
];
export const LIVE_SCRIPTS = ["update", "delete", "reconnecting", "connected", "events", "forbidden", "failed"] as const;
export type LiveScript = typeof LIVE_SCRIPTS[number];
type Watch = { id: string; channel: Pick<Channel<StreamMessage>, "onmessage">; context: string; keys?: WatchResourceKey[]; kind?: string; name?: string; namespace?: string | null };
const watches = new Map<string, Watch>();
let serial = 0;
let changed = false;
let deleted = false;
let events = false;
let failed = false;

export function registerMockWatch(args: Omit<Watch, "id" | "channel"> & { channel?: Pick<Channel<StreamMessage>, "onmessage"> }): string {
    if (!args.channel) throw new Error("Mock watch requires a channel");
    const channel = args.channel;
    const id = `mock-live-${++serial}`;
    watches.set(id, { ...args, channel, id });
    queueMicrotask(() => channel.onmessage({ type: "status", streamId: id, status: "connected", message: "Live updates connected" }));
    return id;
}
export function stopMockWatch(id: string): boolean { return watches.delete(id); }
export function emitMockKindChange(kind: string): void {
    for (const watch of watches.values()) {
        if (watch.keys?.some((key) => key.resourceKind.kind === kind)) {
            watch.channel.onmessage({ type: "resourceChanged", streamId: watch.id, target: { cluster: watch.context, kind }, action: "applied" });
        }
    }
}
export function liveResources(rows: ResourceSummary[]): ResourceSummary[] {
    if (failed) throw new Error("Scripted reload failure");
    return rows.filter((row) => !(deleted && target(row))).map((row) => changed && target(row) && row.healthAssessment ? { ...row, status: "Progressing", healthAssessment: { ...row.healthAssessment, state: "degraded" } } : row);
}
function target(row: ResourceSummary): boolean { return row.kind === "Deployment" && row.namespace === "payments" && row.name === "payments-api"; }
export function liveEvents(kind?: string, name?: string): ResourceEventSummary[] | undefined {
    return events && kind === "Deployment" && name === "payments-api" ? LIVE_EVENTS : undefined;
}
export function replayLiveData(script: LiveScript): void {
    if (script === "failed") failed = true;
    if (script === "connected") failed = false;
    if (script === "update") changed = true;
    if (script === "delete") deleted = true;
    if (script === "events") events = true;
    for (const watch of watches.values()) {
        const streamId = watch.id;
        if (script === "failed") watch.channel.onmessage({ type: "status", streamId, status: "reconnecting", message: "Scripted connection loss" });
        if (script === "connected" || script === "reconnecting" || script === "forbidden") {
            watch.channel.onmessage({ type: "status", streamId, status: script, message: script });
        } else if (script === "events" && watch.name === "payments-api" && watch.kind === "Deployment") {
            watch.channel.onmessage({ type: "resourceEventsChanged", streamId, target: { cluster: watch.context, kind: "Deployment", name: watch.name, namespace: watch.namespace }, action: "applied" });
        } else if (script !== "events" && watch.keys?.some((key) => key.resourceKind.kind === "Deployment" && (!key.namespace || key.namespace === "payments"))) {
            watch.channel.onmessage({ type: "resourceChanged", streamId, target: { cluster: watch.context, kind: "Deployment", name: "payments-api", namespace: "payments" }, action: script === "delete" ? "deleted" : "applied" });
        }
    }
}

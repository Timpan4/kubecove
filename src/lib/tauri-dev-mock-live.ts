import type { Channel } from "@tauri-apps/api/core";
import { mockTiming } from "./tauri-dev-mock-timing";
import { OVERFLOW_EVENTS } from "./tauri-dev-mock-scroll";
import type { ResourceEventSummary, ResourceSummary, ResourceWatchStart, StreamMessage } from "./types";

export const LIVE_EVENTS: ResourceEventSummary[] = [
    { eventType: "Normal", reason: "LiveOlder", message: "Older scripted event", count: 1, lastSeen: "2m", lastSeenAt: "2026-06-29T10:01:00Z", source: "parity", namespace: "payments" },
    { eventType: "Normal", reason: "LiveNewest", message: "Newest scripted event", count: 1, lastSeen: "1m", lastSeenAt: "2026-06-29T10:02:00Z", source: "parity", namespace: "payments" },
];
export const LIVE_SCRIPTS = ["update", "rollout", "delete", "ownership", "reconnecting", "connected", "events", "events-overflow", "forbidden", "failed", "missed-delete"] as const;
export type LiveScript = typeof LIVE_SCRIPTS[number];
type Watch = { id: string; channel: Pick<Channel<StreamMessage>, "onmessage">; context: string; keys?: ResourceWatchStart[]; kind?: string; name?: string; namespace?: string | null };
const watches = new Map<string, Watch>();
const watchStarts: ResourceWatchStart[] = [];
let serial = 0;
let changed = false;
let deleted = false;
// A scripted rollout gives payments-api the "Rolling out" lifecycle, which shows activity.
let rolling = false;
// The "ownership" script moves checkout-worker to a Flux owner. Its ownerRef-only Pod follows it.
let reowned = false;
const REOWNED = "checkout-worker";
const FLUX_OWNER = { provider: "flux", kind: "Kustomization", name: "platform", namespace: "flux-system", confidence: "inventory", provenance: "flux-kustomization-inventory", partial: false };
let events: ResourceEventSummary[] | undefined;
let failed = false;
// Scripted changes so far; the mock list version is one higher, as in tests/common/live_data.rs.
let revision = 0;
// The list version that first shows the scripted change.
let changedAt: number | undefined;

/** The mock list version. A scripted change raises it, as a cluster write does. */
export function mockListVersion(): string { return String(revision + 1); }

/** The version the newest `kind` watch started at, or null when none started. */
export function lastWatchStart(kind: string): { resourceVersion?: string } | null {
    return watchStarts.findLast((start) => start.resourceKind.kind === kind) ?? null;
}

export function registerMockWatch(args: Omit<Watch, "id" | "channel"> & { channel?: Pick<Channel<StreamMessage>, "onmessage"> }): string {
    if (!args.channel) throw new Error("Mock watch requires a channel");
    const channel = args.channel;
    const id = `mock-live-${++serial}`;
    watches.set(id, { ...args, channel, id });
    watchStarts.push(...args.keys ?? []);
    // streams/watch.rs starts at the given list version, or at "0", which replays
    // every object. Either way it delivers a change made after that version.
    const replays = (start: ResourceWatchStart) => changedAt !== undefined
        && (start.resourceVersion === undefined || Number(start.resourceVersion) < changedAt);
    queueMicrotask(() => {
        channel.onmessage({ type: "status", streamId: id, status: "connected", message: "Live updates connected" });
        if (!args.keys?.some((key) => key.resourceKind.kind === "Deployment" && replays(key))) return;
        if (changed || rolling || deleted) {
            channel.onmessage({ type: "resourceChanged", streamId: id, target: { cluster: args.context, kind: "Deployment", name: "payments-api", namespace: "payments" }, action: deleted ? "deleted" : "applied" });
        }
        if (reowned) {
            channel.onmessage({ type: "resourceChanged", streamId: id, target: { cluster: args.context, kind: "Deployment", name: REOWNED, namespace: "payments" }, action: "applied" });
        }
    });
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
    return rows.filter((row) => !(deleted && target(row))).map((row) => {
        if (reowned && row.kind === "Deployment" && row.namespace === "payments" && row.name === REOWNED) return { ...row, gitOpsOwner: FLUX_OWNER };
        if (!target(row)) return row;
        if (rolling) return { ...row, status: "Rolling out" };
        return changed && row.healthAssessment ? { ...row, status: "Progressing", healthAssessment: { ...row.healthAssessment, state: "degraded" } } : row;
    });
}
function target(row: ResourceSummary): boolean { return row.kind === "Deployment" && row.namespace === "payments" && row.name === "payments-api"; }
export function liveEvents(kind?: string, name?: string): ResourceEventSummary[] | undefined {
    return kind === "Deployment" && name === "payments-api" ? events : undefined;
}
export function replayLiveData(script: LiveScript): void {
    // "missed-delete" deletes payments-api without a notification, as an event a lost watch never delivered.
    const missed = script === "missed-delete";
    const rowChange = script === "update" || script === "rollout" || script === "delete" || script === "ownership" || missed;
    if (rowChange) mockTiming.invalidate("mock-dev");
    if (script === "ownership") reowned = true;
    if (script === "failed") failed = true;
    if (script === "connected") failed = false;
    if (script === "update") changed = true;
    if (script === "rollout") rolling = true;
    if (script === "delete" || missed) deleted = true;
    if (missed) {
        revision += 1;
        changedAt = revision + 1;
        return;
    }
    if (rowChange) {
        revision += 1;
        changedAt = revision + 1;
    }
    if (script === "events") events = LIVE_EVENTS;
    if (script === "events-overflow") events = OVERFLOW_EVENTS;
    for (const watch of watches.values()) {
        const streamId = watch.id;
        if (script === "failed") watch.channel.onmessage({ type: "status", streamId, status: "reconnecting", message: "Scripted connection loss" });
        if (script === "connected" || script === "reconnecting" || script === "forbidden") {
            watch.channel.onmessage({ type: "status", streamId, status: script, message: script });
        } else if ((script === "events" || script === "events-overflow") && watch.name === "payments-api" && watch.kind === "Deployment") {
            watch.channel.onmessage({ type: "resourceEventsChanged", streamId, target: { cluster: watch.context, kind: "Deployment", name: watch.name, namespace: watch.namespace }, action: "applied" });
        } else if (script !== "events" && script !== "events-overflow" && watch.keys?.some((key) => key.resourceKind.kind === "Deployment" && (!key.namespace || key.namespace === "payments"))) {
            watch.channel.onmessage({ type: "resourceChanged", streamId, target: { cluster: watch.context, kind: "Deployment", name: script === "ownership" ? REOWNED : "payments-api", namespace: "payments" }, action: script === "delete" ? "deleted" : "applied" });
        }
    }
}

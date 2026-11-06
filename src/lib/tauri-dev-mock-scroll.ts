import type { ResourceEventSummary } from "./types";

// The same event count as benchmarks/resource-detail.bench.ts.
export const OVERFLOW_EVENTS: ResourceEventSummary[] = Array.from(
    { length: 1_000 },
    (_, index) => ({
        eventType: "Normal",
        reason: "Overflow",
        message: `Overflow event ${index}`,
        count: 1,
        lastSeen: "1m",
        lastSeenAt: "2026-06-29T10:02:00Z",
        source: "parity",
        namespace: "payments",
    }),
);

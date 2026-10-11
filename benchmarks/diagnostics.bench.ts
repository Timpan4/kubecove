import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	backendDiagnosticMetricRows,
	diagnosticCounterRows,
	diagnosticMetricTableRows,
	visibleDiagnosticRows,
} from "@/app/svelte/diagnosticsSettingsModel";
import {
	buildDiagnosticsSnapshot,
	type FrontendDiagnosticEvent,
	summarizeDurations,
} from "@/lib/diagnostics";
import { createLatencyReportFromSnapshot } from "@/lib/diagnostics-report";
import type { BackendCacheDiagnosticSnapshot } from "@/lib/diagnostics-types";
import type { BackendDiagnosticEvent } from "@/lib/types";

const frontendEvents: FrontendDiagnosticEvent[] = Array.from(
	{ length: 500 },
	(_, index) => ({
		id: index + 1,
		recordedAt: "2026-10-11T12:00:00Z",
		atMs: 100_000 + index,
		deltaMs: index % 20,
		event: `resources.load.${index % 12}`,
		count: index + 1,
		durationMs: index % 7 === 0 ? undefined : (index % 100) / 3,
		heapMb: index % 2 === 0 ? 128 : undefined,
		data: {
			context: `prod/context-${index % 3}`,
			namespace: `namespace-${index % 100}`,
			ms: index % 7 === 0 ? "unavailable" : index % 100,
		},
	}),
);
const counters = new Map(
	Array.from({ length: 24 }, (_, index) => [
		`surface-${index}.render`,
		index * 10 + 1,
	]),
);
const traceLines = Array.from(
	{ length: 400 },
	(_, index) => `[k8s-debug #${index + 1}] resources.load-${index}`,
);
const backendEvents: BackendDiagnosticEvent[] = Array.from(
	{ length: 100 },
	(_, index) => ({
		id: index + 1,
		recordedAt: "2026-10-11T12:00:00Z",
		command: `list_resources_${index % 8}`,
		status: index % 13 === 0 ? "error" : "ok",
		durationMs: index % 250,
		summary: [{ key: "namespace", value: `namespace-${index % 100}` }],
	}),
);
const backendCaches: BackendCacheDiagnosticSnapshot[] = Array.from(
	{ length: 5 },
	(_, index) => ({
		label: `workspace-${index}`,
		hits: 100 + index,
		misses: index * 2,
		joins: index * 3,
		evictions: index,
		restoredReloadFailures: 0,
		ready: 10 + index,
		dirty: index,
		loading: 0,
		retainedItems: 1_000 + index * 100,
		shallowPayloadBytes: 1_000_000 + index * 50_000,
		weightKind: "shallow-payload-lower-bound",
	}),
);
const snapshot = buildDiagnosticsSnapshot(
	true,
	counters,
	frontendEvents,
	traceLines,
);
const report = createLatencyReportFromSnapshot(snapshot, {
	backendEvents,
	backendCaches,
	generatedAt: "2026-10-11T12:00:00.000Z",
});
// SAFETY: report comes from the typed production serializer; assertions below check its fields.
const parsedReport = JSON.parse(report) as {
	generatedAt: string;
	redacted: boolean;
	frontend: { events: FrontendDiagnosticEvent[] };
};

assert.equal(snapshot.frontendEvents.length, 500);
assert.equal(snapshot.traceLines.length, 400);
assert.equal(parsedReport.generatedAt, "2026-10-11T12:00:00.000Z");
assert.equal(parsedReport.redacted, true);
assert(!report.includes("prod/context-0"));
assert.equal(
	snapshot.summaries.reduce((count, group) => count + group.count, 0),
	428,
);
assert.deepEqual(
	snapshot.summaries.map((group) => group.name),
	[
		"resources.load.0",
		"resources.load.1",
		"resources.load.10",
		"resources.load.11",
		"resources.load.2",
		"resources.load.3",
		"resources.load.4",
		"resources.load.5",
		"resources.load.6",
		"resources.load.7",
		"resources.load.8",
		"resources.load.9",
	],
);
assert.deepEqual(
	summarizeDurations([
		{ name: "rounding", durationMs: 9.999 },
		{ name: "rounding", durationMs: 1.111 },
		{ name: "rounding", durationMs: 2.222 },
	]),
	[{ name: "rounding", count: 3, p50Ms: 2.22, p95Ms: 10, maxMs: 10 }],
);
const displayedMetrics = visibleDiagnosticRows(snapshot.summaries);
assert.equal(displayedMetrics.length, 10);
assert(
	displayedMetrics.every(
		(row, index) =>
			index === 0 ||
			(displayedMetrics[index - 1].p95Ms ?? 0) >= (row.p95Ms ?? 0),
	),
);

describe("diagnostics snapshot and report F54", () => {
	bench("F54 periodic snapshot refresh and visible metric tables", () => {
		const current = buildDiagnosticsSnapshot(
			true,
			counters,
			frontendEvents,
			traceLines,
		);
		visibleDiagnosticRows(current.summaries);
		diagnosticMetricTableRows(current.summaries);
		diagnosticCounterRows(current.counters);
		const backendSummaries = backendDiagnosticMetricRows(backendEvents);
		diagnosticMetricTableRows(backendSummaries);
	});
	bench(
		"F54 explicit report export with fresh snapshot and redacted serialization",
		() => {
			const current = buildDiagnosticsSnapshot(
				true,
				counters,
				frontendEvents,
				traceLines,
			);
			createLatencyReportFromSnapshot(current, {
				backendEvents,
				backendCaches,
				generatedAt: "2026-10-11T12:00:00.000Z",
			});
		},
	);
});

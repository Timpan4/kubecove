import { afterEach, describe, expect, test } from "bun:test";
import {
	clearDiagnostics,
	diagnosticLog,
	getDiagnosticsSnapshot,
	setDiagnosticsEnabled,
	summarizeDurations,
} from "../src/lib/diagnostics";
import { createLatencyReport } from "../src/lib/diagnostics-report";
import type { BackendDiagnosticEvent } from "../src/lib/types";

const backendEvent: BackendDiagnosticEvent = {
	id: 1,
	recordedAt: "2026-06-15T10:00:00Z",
	command: "list_resource_scope",
	status: "ok",
	durationMs: 42,
	summary: [{ key: "rows", value: "12" }],
};

afterEach(() => {
	clearDiagnostics();
	setDiagnosticsEnabled(false);
});

describe("diagnostics", () => {
	test("drops events while disabled", () => {
		setDiagnosticsEnabled(false);

		diagnosticLog("resources.fetch.done", { ms: 12, cluster: "prod" });

		expect(getDiagnosticsSnapshot().frontendEvents).toEqual([]);
	});

	test("records bounded frontend events while enabled", () => {
		setDiagnosticsEnabled(true);

		for (let index = 0; index < 505; index += 1) {
			diagnosticLog("resources.fetch.done", { ms: index });
		}

		const snapshot = getDiagnosticsSnapshot();
		expect(snapshot.frontendEvents).toHaveLength(500);
		expect(snapshot.frontendEvents[0]?.id).toBe(6);
		expect(snapshot.frontendEvents.at(-1)?.durationMs).toBe(504);
	});

	test("summarizes duration percentiles", () => {
		expect(
			summarizeDurations([
				{ name: "a", durationMs: 10 },
				{ name: "a", durationMs: 20 },
				{ name: "a", durationMs: 30 },
				{ name: "b" },
			]),
		).toEqual([
			{ name: "a", count: 3, p50Ms: 20, p95Ms: 30, maxMs: 30 },
			{
				name: "b",
				count: 1,
				p50Ms: undefined,
				p95Ms: undefined,
				maxMs: undefined,
			},
		]);
	});

	test("keeps untimed events out of frontend timing summaries", () => {
		setDiagnosticsEnabled(true);

		diagnosticLog("resources.fetch.start");
		diagnosticLog("resources.fetch.done", { ms: 24 });
		diagnosticLog("resources.render");

		const snapshot = getDiagnosticsSnapshot();

		expect(snapshot.summaries).toEqual([
			{
				name: "resources.fetch.done",
				count: 1,
				p50Ms: 24,
				p95Ms: 24,
				maxMs: 24,
			},
		]);
		expect(snapshot.counters["resources.fetch.start"]).toBe(1);
		expect(snapshot.counters["resources.render"]).toBe(1);
	});

	test("copies redacted latency reports by default", () => {
		setDiagnosticsEnabled(true);
		diagnosticLog("resources.fetch.done", {
			ms: 18,
			cluster: "prod-west",
			namespace: "payments",
			name: "api-0",
			error: "api-0 failed",
		});

		const report = createLatencyReport({ backendEvents: [backendEvent] });

		expect(report).toContain('"redacted": true');
		expect(report).toContain("context-1");
		expect(report).toContain("namespace-1");
		expect(report).toContain("resource-1");
		expect(report).not.toContain("prod-west");
		expect(report).not.toContain("payments");
		expect(report).not.toContain("api-0 failed");
	});

	test("can include identifiers for trusted local debugging", () => {
		setDiagnosticsEnabled(true);
		diagnosticLog("resources.fetch.done", {
			ms: 18,
			cluster: "prod-west",
			namespace: "payments",
			name: "api-0",
		});

		const report = createLatencyReport({
			backendEvents: [backendEvent],
			includeIdentifiers: true,
		});

		expect(report).toContain('"redacted": false');
		expect(report).toContain("prod-west");
		expect(report).toContain("payments");
		expect(report).toContain("api-0");
	});
});

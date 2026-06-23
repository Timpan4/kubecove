import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	backendDiagnosticMetricRows,
	diagnosticCounterRows,
	diagnosticMetricTableRows,
} from "../src/app/svelte/diagnosticsSettingsModel";
import type { BackendDiagnosticEvent } from "../src/lib/types";

describe("svelte diagnostics settings model", () => {
	test("sorts timing rows by slowest p95 then formats missing values", () => {
		expect(
			diagnosticMetricTableRows([
				{ name: "fast", count: 2, p50Ms: 2, p95Ms: 4, maxMs: 5 },
				{ name: "slow", count: 1, p50Ms: 10, p95Ms: 20, maxMs: 30 },
				{ name: "empty", count: 1 },
			]),
		).toEqual([
			["slow", "1", "10 ms", "20 ms", "30 ms"],
			["fast", "2", "2 ms", "4 ms", "5 ms"],
		]);
	});

	test("keeps render counters only and sorts by count", () => {
		expect(
			diagnosticCounterRows({
				"app.render": 2,
				"resource.fetch": 9,
				"detail.render": 5,
			}),
		).toEqual([
			["detail.render", "5"],
			["app.render", "2"],
		]);
	});

	test("summarizes backend diagnostic events by command", () => {
		const events: BackendDiagnosticEvent[] = [
			event("list_resources", 40),
			event("list_resources", 20),
			event("get_yaml", 10),
		];
		expect(backendDiagnosticMetricRows(events)).toContainEqual({
			name: "list_resources",
			count: 2,
			p50Ms: 20,
			p95Ms: 40,
			maxMs: 40,
		});
	});

	test("Svelte diagnostics settings reads backend events through svelte-query", () => {
		const source = readFileSync(
			"src/app/svelte/DiagnosticsSettings.svelte",
			"utf8",
		);

		expect(source).toContain('from "@tanstack/svelte-query"');
		expect(source).toContain("createQuery<BackendDiagnosticEvent[]>");
		expect(source).toContain("queryKeys.backendDiagnostics()");
		expect(source).toContain("queryFn: () => getBackendDiagnostics(client)");
		expect(source).toContain("refetchInterval: settings.debugModeEnabled ? 1_500 : false");
		expect(source).toContain("queryClient.setQueryData(queryKeys.backendDiagnostics(), [])");
		expect(source).not.toContain("backendRefreshPending");
		expect(source).not.toContain("let backendEvents = $state");
	});
});

function event(command: string, durationMs: number): BackendDiagnosticEvent {
	return {
		id: durationMs,
		recordedAt: "2026-01-01T00:00:00.000Z",
		command,
		status: "ok",
		durationMs,
		summary: [],
	};
}

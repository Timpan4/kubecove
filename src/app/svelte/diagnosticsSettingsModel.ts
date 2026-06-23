import type { DiagnosticMetricSummary } from "@/lib/diagnostics";
import type { BackendDiagnosticEvent } from "@/lib/types";

export function formatDiagnosticMs(value: number | undefined): string {
	return typeof value === "number" ? `${value} ms` : "-";
}

export function visibleDiagnosticRows(
	rows: DiagnosticMetricSummary[],
	maxRows = 10,
): DiagnosticMetricSummary[] {
	return [...rows]
		.filter((row) => row.count > 0 && typeof row.maxMs === "number")
		.sort((a, b) => {
			const aScore = a.p95Ms ?? a.maxMs ?? a.p50Ms ?? 0;
			const bScore = b.p95Ms ?? b.maxMs ?? b.p50Ms ?? 0;
			return bScore - aScore || a.name.localeCompare(b.name);
		})
		.slice(0, maxRows);
}

export function diagnosticMetricTableRows(
	rows: DiagnosticMetricSummary[],
): string[][] {
	return visibleDiagnosticRows(rows).map((row) => [
		row.name,
		String(row.count),
		formatDiagnosticMs(row.p50Ms),
		formatDiagnosticMs(row.p95Ms),
		formatDiagnosticMs(row.maxMs),
	]);
}

export function diagnosticCounterRows(counters: Record<string, number>): string[][] {
	return Object.entries(counters)
		.filter(([name]) => name.endsWith(".render"))
		.sort(([aName, aCount], [bName, bCount]) => {
			return bCount - aCount || aName.localeCompare(bName);
		})
		.slice(0, 8)
		.map(([name, count]) => [name, String(count)]);
}

export function backendDiagnosticMetricRows(
	events: BackendDiagnosticEvent[],
): DiagnosticMetricSummary[] {
	const groups = new Map<string, number[]>();
	for (const event of events) {
		const durations = groups.get(event.command) ?? [];
		durations.push(event.durationMs);
		groups.set(event.command, durations);
	}
	return [...groups.entries()].map(([name, durations]) => {
		const sorted = [...durations].sort((a, b) => a - b);
		return {
			name,
			count: sorted.length,
			p50Ms: percentile(sorted, 50),
			p95Ms: percentile(sorted, 95),
			maxMs: sorted.at(-1),
		};
	});
}

function percentile(sorted: number[], percentileValue: number): number | undefined {
	if (sorted.length === 0) return undefined;
	const index = Math.min(
		sorted.length - 1,
		Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1),
	);
	return sorted[index];
}

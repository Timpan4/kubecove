import type { AppUsageMemory, AppUsageMetrics, AppUsageMetricsBreakdown } from "./types";

export interface FlattenedUsageMetricsBreakdown {
	item: AppUsageMetricsBreakdown;
	depth: number;
}

export interface UsageMemoryFigure {
	key: keyof AppUsageMemory;
	label: string;
	description: string;
}

export const USAGE_MEMORY_FIGURES: readonly UsageMemoryFigure[] = [
	{
		key: "wssBytes",
		label: "WSS",
		description:
			"Working set size. Windows reports its working set. Linux reports memory touched since the previous refresh when working set measurement is on. Not available on macOS.",
	},
	{
		key: "rssBytes",
		label: "RSS",
		description: "Resident set size on Linux and macOS. Pages shared between processes count once per process.",
	},
	{
		key: "pssBytes",
		label: "PSS",
		description: "Proportional set size on Linux. Shared pages are split between the processes that map them.",
	},
];

export function formatCpuPercent(value: number): string {
	if (!Number.isFinite(value)) return "--";
	const clamped = Math.min(100, Math.max(0, value));
	return `${clamped.toFixed(1)}%`;
}

export function formatMemoryBytes(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes < 0) return "--";
	const units = ["B", "KB", "MB", "GB"] as const;
	let value = bytes;
	let unitIndex = 0;

	while (value >= 1024 && unitIndex < units.length - 1) {
		value /= 1024;
		unitIndex += 1;
	}

	const precision = unitIndex === 0 || value >= 10 ? 0 : 1;
	return `${value.toFixed(precision)} ${units[unitIndex]}`;
}

/** Unreported figures render as "--", like other unavailable usage values. */
export function formatMemoryFigure(bytes: number | null): string {
	return bytes === null ? "--" : formatMemoryBytes(bytes);
}

export function formatProcessCount(count: number): string {
	if (!Number.isFinite(count) || count < 0) return "--";
	const normalized = Math.floor(count);
	return `${normalized} ${normalized === 1 ? "process" : "processes"}`;
}

function formatMemoryFigures(memory: AppUsageMemory): string[] {
	return USAGE_MEMORY_FIGURES.map((figure) => `${figure.label} ${formatMemoryFigure(memory[figure.key])}`);
}

export function formatUsageMetrics(metrics: AppUsageMetrics): string {
	return [
		`CPU ${formatCpuPercent(metrics.cpuPercent)}`,
		...formatMemoryFigures(metrics.memory),
		formatProcessCount(metrics.processCount),
	].join(" · ");
}

/** Explains each memory figure, and the interval Linux WSS covers when it is measured. */
export function describeUsageMemory(metrics: AppUsageMetrics): string {
	const lines = USAGE_MEMORY_FIGURES.map((figure) => `${figure.label}: ${figure.description}`);
	if (metrics.workingSetWindowMs !== null) {
		lines.push(`WSS covers the last ${(metrics.workingSetWindowMs / 1000).toFixed(1)} s.`);
	}
	return lines.join("\n");
}

export function formatUsageMetricsBreakdownDetails(
	item: AppUsageMetricsBreakdown,
): string {
	return [
		`CPU ${formatCpuPercent(item.cpuPercent)}`,
		...formatMemoryFigures(item.memory),
		formatProcessCount(item.processCount),
	].join(" · ");
}

export function flattenUsageMetricsBreakdown(
	items: AppUsageMetricsBreakdown[],
	depth = 0,
): FlattenedUsageMetricsBreakdown[] {
	const flattened: FlattenedUsageMetricsBreakdown[] = [];
	const pending: FlattenedUsageMetricsBreakdown[] = [];
	for (let index = items.length - 1; index >= 0; index -= 1) {
		pending.push({ item: items[index], depth });
	}
	while (pending.length > 0) {
		const current = pending.pop();
		if (!current) continue;
		flattened.push(current);
		for (let index = current.item.children.length - 1; index >= 0; index -= 1) {
			pending.push({ item: current.item.children[index], depth: current.depth + 1 });
		}
	}
	return flattened;
}

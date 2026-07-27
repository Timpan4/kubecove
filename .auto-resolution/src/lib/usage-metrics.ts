import type { AppUsageMetrics, AppUsageMetricsBreakdown } from "./types";

export interface FlattenedUsageMetricsBreakdown {
	item: AppUsageMetricsBreakdown;
	depth: number;
}

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

export function formatProcessCount(count: number): string {
	if (!Number.isFinite(count) || count < 0) return "--";
	const normalized = Math.floor(count);
	return `${normalized} ${normalized === 1 ? "process" : "processes"}`;
}

export function formatUsageMetrics(metrics: AppUsageMetrics): string {
	return [
		`CPU ${formatCpuPercent(metrics.cpuPercent)}`,
		`Memory ${formatMemoryBytes(metrics.memoryBytes)}`,
		formatProcessCount(metrics.processCount),
	].join(" · ");
}

export function formatUsageMetricsBreakdown(
	item: AppUsageMetricsBreakdown,
): string {
	return [item.label, formatUsageMetricsBreakdownDetails(item)].join(" · ");
}

export function formatUsageMetricsBreakdownDetails(
	item: AppUsageMetricsBreakdown,
): string {
	return [
		`CPU ${formatCpuPercent(item.cpuPercent)}`,
		formatMemoryBytes(item.memoryBytes),
		formatProcessCount(item.processCount),
	].join(" · ");
}

export function flattenUsageMetricsBreakdown(
	items: AppUsageMetricsBreakdown[],
	depth = 0,
): FlattenedUsageMetricsBreakdown[] {
	return items.flatMap((item) => [
		{ item, depth },
		...flattenUsageMetricsBreakdown(item.children, depth + 1),
	]);
}

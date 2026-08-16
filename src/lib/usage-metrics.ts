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

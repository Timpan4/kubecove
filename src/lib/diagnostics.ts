export type DiagnosticValue = string | number | boolean | null | undefined;
export type DiagnosticData = Record<string, DiagnosticValue>;

export interface FrontendDiagnosticEvent {
	id: number;
	recordedAt: string;
	atMs: number;
	deltaMs: number;
	event: string;
	count: number;
	durationMs?: number;
	heapMb?: number;
	data: DiagnosticData;
}

export interface DiagnosticMetricSummary {
	name: string;
	count: number;
	p50Ms?: number;
	p95Ms?: number;
	maxMs?: number;
}

export interface DiagnosticsSnapshot {
	enabled: boolean;
	frontendEvents: FrontendDiagnosticEvent[];
	counters: Record<string, number>;
	summaries: DiagnosticMetricSummary[];
	traceLines: string[];
}

type PerformanceWithMemory = Performance & {
	memory?: {
		usedJSHeapSize?: number;
		totalJSHeapSize?: number;
		jsHeapSizeLimit?: number;
	};
};

type DebugWindow = Window & {
	__K8S_DEBUG_TRACE__?: string[];
};

const MAX_TRACE_LINES = 400;
const MAX_FRONTEND_EVENTS = 500;
const CHATTER_EVENTS = new Set(["app.render", "resources.render", "detail.render"]);

let enabled = false;
let sequence = 0;
let previousTimestamp = performance.now();
const counters = new Map<string, number>();
const frontendEvents: FrontendDiagnosticEvent[] = [];
const traceLines: string[] = [];

function formatBytes(bytes: number | undefined): number | undefined {
	if (bytes === undefined) return undefined;
	return Math.round(bytes / 1024 / 1024);
}

function sanitizeValue(value: DiagnosticValue): string {
	if (value === undefined) return "undefined";
	if (value === null) return "null";
	return String(value);
}

function serializeData(data: DiagnosticData | undefined): string {
	if (!data) return "";
	return Object.entries(data)
		.map(([key, value]) => `${key}=${sanitizeValue(value)}`)
		.join(" ");
}

function pushBounded<T>(items: T[], item: T, max: number): void {
	items.push(item);
	if (items.length > max) {
		items.splice(0, items.length - max);
	}
}

function setWindowTrace(lines: string[]): void {
	if (typeof window === "undefined") return;
	const debugWindow = window as DebugWindow;
	debugWindow.__K8S_DEBUG_TRACE__ = [...lines];
}

function durationFromData(data: DiagnosticData | undefined): number | undefined {
	return typeof data?.ms === "number" ? data.ms : undefined;
}

function rounded(value: number): number {
	return Number(value.toFixed(2));
}

function percentile(sorted: number[], percentileValue: number): number | undefined {
	if (sorted.length === 0) return undefined;
	const index = Math.min(
		sorted.length - 1,
		Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1),
	);
	return rounded(sorted[index] ?? 0);
}

export function summarizeDurations(
	entries: Array<{ name: string; durationMs?: number }>,
): DiagnosticMetricSummary[] {
	const groups = new Map<string, { count: number; durations: number[] }>();
	for (const entry of entries) {
		const group = groups.get(entry.name) ?? { count: 0, durations: [] };
		group.count += 1;
		if (typeof entry.durationMs === "number") {
			group.durations.push(entry.durationMs);
		}
		groups.set(entry.name, group);
	}

	return [...groups.entries()]
		.map(([name, group]) => {
			const durations = [...group.durations].sort((a, b) => a - b);
			return {
				name,
				count: group.count,
				p50Ms: percentile(durations, 50),
				p95Ms: percentile(durations, 95),
				maxMs:
					durations.length > 0
						? rounded(durations[durations.length - 1] ?? 0)
						: undefined,
			};
		})
		.sort((a, b) => a.name.localeCompare(b.name));
}

export function diagnosticsEnabled(): boolean {
	return enabled;
}

export function setDiagnosticsEnabled(nextEnabled: boolean): void {
	enabled = nextEnabled;
	previousTimestamp = performance.now();
	if (!nextEnabled) {
		setWindowTrace(traceLines);
	}
}

export function clearDiagnostics(): void {
	sequence = 0;
	previousTimestamp = performance.now();
	counters.clear();
	frontendEvents.length = 0;
	traceLines.length = 0;
	setWindowTrace(traceLines);
}

export function getDiagnosticsSnapshot(): DiagnosticsSnapshot {
	const counterSnapshot = Object.fromEntries(counters.entries());
	const timedEvents = frontendEvents.filter(
		(event) => typeof event.durationMs === "number",
	);
	return {
		enabled,
		frontendEvents: [...frontendEvents],
		counters: counterSnapshot,
		summaries: summarizeDurations(
			timedEvents.map((event) => ({
				name: event.event,
				durationMs: event.durationMs,
			})),
		),
		traceLines: [...traceLines],
	};
}

export function diagnosticLog(event: string, data?: DiagnosticData): void {
	if (!enabled) return;

	const now = performance.now();
	const count = (counters.get(event) ?? 0) + 1;
	counters.set(event, count);

	const memory = (performance as PerformanceWithMemory).memory;
	const heapMb = formatBytes(memory?.usedJSHeapSize);
	const delta = Math.round(now - previousTimestamp);
	previousTimestamp = now;
	sequence += 1;

	const eventRecord: FrontendDiagnosticEvent = {
		id: sequence,
		recordedAt: new Date().toISOString(),
		atMs: Math.round(now),
		deltaMs: delta,
		event,
		count,
		durationMs: durationFromData(data),
		heapMb,
		data: data ?? {},
	};
	pushBounded(frontendEvents, eventRecord, MAX_FRONTEND_EVENTS);

	const fields = serializeData(data);
	const line = `[k8s-debug #${sequence} +${delta}ms count=${count}${
		heapMb === undefined ? "" : ` heap=${heapMb}MB`
	}] ${event}${fields ? ` ${fields}` : ""}`;
	pushBounded(traceLines, line, MAX_TRACE_LINES);
	setWindowTrace(traceLines);

	if (!CHATTER_EVENTS.has(event) || count <= 20 || count % 100 === 0) {
		console.info(line);
	}
}

export function diagnosticResultSummary(value: unknown): string {
	if (Array.isArray(value)) return `array:${value.length}`;
	if (typeof value === "string") return `string:${value.length}`;
	if (value && typeof value === "object") return "object";
	return typeof value;
}

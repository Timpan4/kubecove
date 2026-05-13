type DiagnosticValue = string | number | boolean | null | undefined;
type DiagnosticData = Record<string, DiagnosticValue>;

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
const CHATTER_EVENTS = new Set(["app.render", "resources.render", "detail.render"]);

const counters = new Map<string, number>();
let sequence = 0;
let previousTimestamp = performance.now();

function formatBytes(bytes: number | undefined): string | undefined {
	if (bytes === undefined) return undefined;
	return `${Math.round(bytes / 1024 / 1024)}MB`;
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

export function diagnosticLog(event: string, data?: DiagnosticData): void {
	const now = performance.now();
	const count = (counters.get(event) ?? 0) + 1;
	counters.set(event, count);

	const memory = (performance as PerformanceWithMemory).memory;
	const heap = formatBytes(memory?.usedJSHeapSize);
	const delta = Math.round(now - previousTimestamp);
	previousTimestamp = now;
	sequence += 1;

	const fields = serializeData(data);
	const line = `[k8s-debug #${sequence} +${delta}ms count=${count}${heap ? ` heap=${heap}` : ""}] ${event}${fields ? ` ${fields}` : ""}`;

	const debugWindow = window as DebugWindow;
	const trace = debugWindow.__K8S_DEBUG_TRACE__ ?? [];
	trace.push(line);
	if (trace.length > MAX_TRACE_LINES) {
		trace.splice(0, trace.length - MAX_TRACE_LINES);
	}
	debugWindow.__K8S_DEBUG_TRACE__ = trace;

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

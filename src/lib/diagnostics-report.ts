import { APP_VERSION } from "./app-version";
import {
	getDiagnosticsSnapshot,
	summarizeDurations,
	type DiagnosticData,
	type DiagnosticMetricSummary,
	type FrontendDiagnosticEvent,
} from "./diagnostics";
import type { BackendDiagnosticEvent } from "./types";

export interface LatencyReport {
	generatedAt: string;
	appVersion: string;
	redacted: boolean;
	frontend: {
		summaries: DiagnosticMetricSummary[];
		counters: Record<string, number>;
		events: FrontendDiagnosticEvent[];
	};
	backend: {
		summaries: DiagnosticMetricSummary[];
		events: BackendDiagnosticEvent[];
	};
}

const REDACT_KEYS = new Set([
	"cluster",
	"clusterContext",
	"context",
	"namespace",
	"namespaces",
	"name",
	"key",
	"target",
	"podName",
	"resource",
	"sessionId",
	"streamId",
]);
const OMIT_KEYS = new Set(["error", "message", "componentStack"]);

function aliasFactory() {
	const aliases = new Map<string, Map<string, string>>();
	return (kind: string, value: string): string => {
		if (!value) return value;
		const values = aliases.get(kind) ?? new Map<string, string>();
		if (!aliases.has(kind)) aliases.set(kind, values);
		const existing = values.get(value);
		if (existing) return existing;
		const alias = `${kind}-${values.size + 1}`;
		values.set(value, alias);
		return alias;
	};
}

function aliasKindFor(key: string): string {
	if (key === "cluster" || key === "clusterContext" || key === "context") {
		return "context";
	}
	if (key === "namespace" || key === "namespaces") {
		return "namespace";
	}
	return "resource";
}

function redactData(
	data: DiagnosticData,
	aliasFor: (kind: string, value: string) => string,
): DiagnosticData {
	const redacted: DiagnosticData = {};
	for (const [key, value] of Object.entries(data)) {
		if (OMIT_KEYS.has(key)) {
			redacted[key] = "[redacted]";
			continue;
		}
		if (typeof value === "string" && REDACT_KEYS.has(key)) {
			redacted[key] = aliasFor(aliasKindFor(key), value);
			continue;
		}
		redacted[key] = value;
	}
	return redacted;
}

function redactFrontendEvents(
	events: FrontendDiagnosticEvent[],
	includeIdentifiers: boolean,
): FrontendDiagnosticEvent[] {
	if (includeIdentifiers) return events;
	const aliasFor = aliasFactory();
	return events.map((event) => ({
		...event,
		data: redactData(event.data, aliasFor),
	}));
}

function backendSummaries(
	events: BackendDiagnosticEvent[],
): DiagnosticMetricSummary[] {
	return summarizeDurations(
		events.map((event) => ({
			name: event.command,
			durationMs: event.durationMs,
		})),
	);
}

export function createLatencyReport({
	backendEvents = [],
	includeIdentifiers = false,
}: {
	backendEvents?: BackendDiagnosticEvent[];
	includeIdentifiers?: boolean;
} = {}): string {
	const snapshot = getDiagnosticsSnapshot();
	const report: LatencyReport = {
		generatedAt: new Date().toISOString(),
		appVersion: APP_VERSION,
		redacted: !includeIdentifiers,
		frontend: {
			summaries: snapshot.summaries,
			counters: snapshot.counters,
			events: redactFrontendEvents(snapshot.frontendEvents, includeIdentifiers),
		},
		backend: {
			summaries: backendSummaries(backendEvents),
			events: backendEvents,
		},
	};
	return JSON.stringify(report, null, 2);
}

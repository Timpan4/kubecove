export type BackendDiagnosticStatus = "ok" | "error" | "cancelled";

export interface BackendCacheDiagnosticSnapshot {
	label: string;
	hits: number;
	misses: number;
	joins: number;
	evictions: number;
	restoredReloadFailures: number;
	ready: number;
	dirty: number;
	loading: number;
	retainedItems: number;
	shallowPayloadBytes: number;
	weightKind: "shallow-payload-lower-bound";
}

export interface BackendDiagnosticField {
	key: string;
	value: string;
}

export interface BackendDiagnosticEvent {
	id: number;
	recordedAt: string;
	command: string;
	status: BackendDiagnosticStatus;
	durationMs: number;
	summary: BackendDiagnosticField[];
}

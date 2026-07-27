export type BackendDiagnosticStatus = "ok" | "error" | "cancelled";

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

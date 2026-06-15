import { useEffect, useMemo, useRef, useState } from "react";
import { Copy, RefreshCcw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { ToggleButton } from "@/components/ToggleButton";
import {
	clearDiagnostics,
	getDiagnosticsSnapshot,
	summarizeDurations,
	type DiagnosticMetricSummary,
	type DiagnosticsSnapshot,
} from "@/lib/diagnostics";
import { createLatencyReport } from "@/lib/diagnostics-report";
import { useSettingsState } from "@/lib/settings";
import {
	clearBackendDiagnostics,
	createTauriClient,
	getBackendDiagnostics,
} from "@/lib/tauri";
import type { BackendDiagnosticEvent } from "@/lib/types";
import type { SettingsRowMeta } from "./SettingsControls";
import { SettingsBlock, SettingsRow, SettingsSection } from "./SettingsControls";

export const DIAGNOSTICS_SETTINGS_ROWS = {
	debugMode: {
		title: "Enable diagnostics",
		description:
			"Collects local latency timings for release smoke testing. Trace data stays in memory.",
	},
	report: {
		title: "Latency report",
		description:
			"Shows frontend and backend timing summaries, with redacted copy output by default.",
	},
} satisfies Record<string, SettingsRowMeta>;

function formatMs(value: number | undefined): string {
	return typeof value === "number" ? `${value} ms` : "-";
}

function emptySnapshot(): DiagnosticsSnapshot {
	return getDiagnosticsSnapshot();
}

function SummaryTable({
	title,
	rows,
	emptyText = "No timings yet.",
	maxRows = 10,
}: {
	title: string;
	rows: DiagnosticMetricSummary[];
	emptyText?: string;
	maxRows?: number;
}) {
	const visibleRows = [...rows]
		.filter((row) => row.count > 0 && typeof row.maxMs === "number")
		.sort((a, b) => {
			const aScore = a.p95Ms ?? a.maxMs ?? a.p50Ms ?? 0;
			const bScore = b.p95Ms ?? b.maxMs ?? b.p50Ms ?? 0;
			return bScore - aScore || a.name.localeCompare(b.name);
		})
		.slice(0, maxRows);
	return (
		<div className="flex flex-col gap-2">
			<div className="flex items-center justify-between gap-3">
				<div className="text-xs font-medium text-foreground">{title}</div>
				{visibleRows.length > 0 && (
					<div className="text-xs text-muted-foreground">
						Slowest {visibleRows.length}
					</div>
				)}
			</div>
			<div className="max-h-72 overflow-auto">
				<Table className="min-w-[34rem]">
					<TableHeader>
						<TableRow>
							<TableHead>Name</TableHead>
							<TableHead className="text-right">Count</TableHead>
							<TableHead className="text-right">p50</TableHead>
							<TableHead className="text-right">p95</TableHead>
							<TableHead className="text-right">Max</TableHead>
						</TableRow>
					</TableHeader>
					<TableBody>
						{visibleRows.length === 0 ? (
							<TableRow>
								<TableCell colSpan={5} className="text-muted-foreground">
									{emptyText}
								</TableCell>
							</TableRow>
						) : (
							visibleRows.map((row) => (
								<TableRow key={row.name}>
									<TableCell className="max-w-72 truncate font-mono">
										{row.name}
									</TableCell>
									<TableCell className="text-right">{row.count}</TableCell>
									<TableCell className="text-right">
										{formatMs(row.p50Ms)}
									</TableCell>
									<TableCell className="text-right">
										{formatMs(row.p95Ms)}
									</TableCell>
									<TableCell className="text-right">
										{formatMs(row.maxMs)}
									</TableCell>
								</TableRow>
							))
						)}
					</TableBody>
				</Table>
			</div>
		</div>
	);
}

function CounterTable({ counters }: { counters: Record<string, number> }) {
	const rows = Object.entries(counters)
		.filter(([name]) => name.endsWith(".render"))
		.sort(([aName, aCount], [bName, bCount]) => {
			return bCount - aCount || aName.localeCompare(bName);
		})
		.slice(0, 8);

	return (
		<div className="flex flex-col gap-2">
			<div className="text-xs font-medium text-foreground">Render counters</div>
			<Table className="min-w-[20rem]">
				<TableHeader>
					<TableRow>
						<TableHead>Name</TableHead>
						<TableHead className="text-right">Count</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{rows.length === 0 ? (
						<TableRow>
							<TableCell colSpan={2} className="text-muted-foreground">
								No render counts yet.
							</TableCell>
						</TableRow>
					) : (
						rows.map(([name, count]) => (
							<TableRow key={name}>
								<TableCell className="max-w-72 truncate font-mono">
									{name}
								</TableCell>
								<TableCell className="text-right">{count}</TableCell>
							</TableRow>
						))
					)}
				</TableBody>
			</Table>
		</div>
	);
}

function backendSummaries(events: BackendDiagnosticEvent[]) {
	return summarizeDurations(
		events.map((event) => ({
			name: event.command,
			durationMs: event.durationMs,
		})),
	);
}

function errorStatus(prefix: string, error: unknown): string {
	return `${prefix}: ${error instanceof Error ? error.message : String(error)}`;
}

export function DiagnosticsSection({ showTitle }: { showTitle: boolean }) {
	const debugModeEnabled = useSettingsState((state) => state.debugModeEnabled);
	const setDebugModeEnabled = useSettingsState(
		(state) => state.setDebugModeEnabled,
	);
	const client = useMemo(() => createTauriClient(), []);
	const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot>(emptySnapshot);
	const [backendEvents, setBackendEvents] = useState<BackendDiagnosticEvent[]>([]);
	const [includeIdentifiers, setIncludeIdentifiers] = useState(false);
	const [status, setStatus] = useState("");
	const backendRefreshPendingRef = useRef(false);

	useEffect(() => {
		let cancelled = false;
		const refresh = () => {
			setSnapshot(getDiagnosticsSnapshot());
			if (!debugModeEnabled || backendRefreshPendingRef.current) return;
			backendRefreshPendingRef.current = true;
			void getBackendDiagnostics(client)
				.then((events) => {
					if (!cancelled) setBackendEvents(events);
				})
				.catch((error) => {
					if (!cancelled) {
						setStatus(errorStatus("Could not refresh diagnostics", error));
					}
				})
				.finally(() => {
					backendRefreshPendingRef.current = false;
				});
		};
		refresh();
		if (!debugModeEnabled) return () => {
			cancelled = true;
		};
		const intervalId = window.setInterval(refresh, 1_500);
		return () => {
			cancelled = true;
			window.clearInterval(intervalId);
		};
	}, [client, debugModeEnabled]);

	const clearTrace = async () => {
		clearDiagnostics();
		setSnapshot(getDiagnosticsSnapshot());
		setBackendEvents([]);
		try {
			await clearBackendDiagnostics(client);
			setStatus("Trace cleared.");
		} catch (error) {
			setStatus(errorStatus("Could not clear diagnostics", error));
		}
	};

	const refreshTrace = async () => {
		setSnapshot(getDiagnosticsSnapshot());
		if (backendRefreshPendingRef.current) return;
		backendRefreshPendingRef.current = true;
		try {
			const events = await getBackendDiagnostics(client);
			setBackendEvents(events);
			setStatus("Trace refreshed.");
		} catch (error) {
			setStatus(errorStatus("Could not refresh diagnostics", error));
		} finally {
			backendRefreshPendingRef.current = false;
		}
	};

	const copyReport = async () => {
		try {
			const events = await getBackendDiagnostics(client);
			const report = createLatencyReport({
				backendEvents: events,
				includeIdentifiers,
			});
			await navigator.clipboard.writeText(report);
			setBackendEvents(events);
			setStatus(includeIdentifiers ? "Report copied." : "Redacted report copied.");
		} catch (error) {
			setStatus(errorStatus("Could not copy diagnostics report", error));
		}
	};

	return (
		<SettingsSection title="Diagnostics" showTitle={showTitle}>
			<SettingsRow {...DIAGNOSTICS_SETTINGS_ROWS.debugMode}>
				<ToggleButton
					checked={debugModeEnabled}
					onCheckedChange={setDebugModeEnabled}
					ariaLabel={DIAGNOSTICS_SETTINGS_ROWS.debugMode.title}
				/>
			</SettingsRow>
			<SettingsBlock
				{...DIAGNOSTICS_SETTINGS_ROWS.report}
				actions={
					<div className="flex items-center gap-2">
						<Button
							type="button"
							variant="outline"
							size="sm"
							onClick={refreshTrace}
						>
							<RefreshCcw data-icon="inline-start" />
							Refresh
						</Button>
						<Button type="button" variant="outline" size="sm" onClick={clearTrace}>
							<Trash2 data-icon="inline-start" />
							Clear
						</Button>
						<Button type="button" size="sm" onClick={copyReport}>
							<Copy data-icon="inline-start" />
							Copy
						</Button>
					</div>
				}
			>
				<div className="flex flex-col gap-4">
					<div className="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
						<span>
							Frontend {snapshot.frontendEvents.length} events, backend{" "}
							{backendEvents.length} events.
						</span>
						<div className="flex items-center gap-2">
							<span>Include identifiers</span>
							<ToggleButton
								checked={includeIdentifiers}
								onCheckedChange={setIncludeIdentifiers}
								ariaLabel="Include identifiers in copied latency report"
							/>
						</div>
					</div>
					<SummaryTable
						title="Frontend timings"
						rows={snapshot.summaries}
						emptyText="No timed frontend spans yet."
					/>
					<CounterTable counters={snapshot.counters} />
					<SummaryTable
						title="Backend timings"
						rows={backendSummaries(backendEvents)}
						emptyText="No backend timings yet."
					/>
					{status && <div className="text-xs text-muted-foreground">{status}</div>}
				</div>
			</SettingsBlock>
		</SettingsSection>
	);
}

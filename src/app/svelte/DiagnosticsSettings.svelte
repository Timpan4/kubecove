<script lang="ts">
	import { onDestroy, onMount } from "svelte";
	import { createQuery, useQueryClient } from "@tanstack/svelte-query";
	import { Copy, ExternalLink, RefreshCcw, Trash2 } from "lucide-svelte";
	import { Button, Checkbox, FieldGroup } from "@/components/ui/svelte";
	import {
		clearDiagnostics,
		getDiagnosticsSnapshot,
		type DiagnosticsSnapshot,
	} from "@/lib/diagnostics";
	import { createLatencyReport } from "@/lib/diagnostics-report";
	import {
		clearBackendDiagnostics,
		createTauriClient,
		getBackendDiagnostics,
	} from "@/lib/tauri";
	import { queryKeys } from "@/lib/queryKeys";
	import type { BackendDiagnosticEvent } from "@/lib/types";
	import SettingsRow from "./SettingsRow.svelte";
	import SimpleTable from "./SimpleTable.svelte";
	import {
		backendDiagnosticMetricRows,
		diagnosticCounterRows,
		diagnosticMetricTableRows,
	} from "./diagnosticsSettingsModel";
	import { settingsStore } from "@/lib/settings-store";

	const TOPOLOGY_SPIKE_URL =
		"/src/features/resources/topology-spike/svelte.html?nodes=4000&viewport=focused&nodeDetail=compact&edgeMode=selected&edgeType=straight";

	const client = createTauriClient();
	const queryClient = useQueryClient();
	let snapshot = $state<DiagnosticsSnapshot>(getDiagnosticsSnapshot());
	let includeIdentifiers = $state(false);
	let status = $state("");
	let intervalId: number | null = null;
	const settings = $derived($settingsStore);
	const backendDiagnosticsQuery = createQuery<BackendDiagnosticEvent[]>(() => ({
		queryKey: queryKeys.backendDiagnostics(),
		queryFn: () => getBackendDiagnostics(client),
		enabled: settings.debugModeEnabled,
		refetchInterval: settings.debugModeEnabled ? 1_500 : false,
		staleTime: 1_500,
	}));

	const backendEvents = $derived(backendDiagnosticsQuery.data ?? []);
	const frontendRows = $derived(diagnosticMetricTableRows(snapshot.summaries));
	const counterRows = $derived(diagnosticCounterRows(snapshot.counters));
	const backendRows = $derived(
		diagnosticMetricTableRows(backendDiagnosticMetricRows(backendEvents)),
	);

	onMount(() => {
		void refreshTrace(false);
		intervalId = window.setInterval(() => {
			snapshot = getDiagnosticsSnapshot();
		}, 1_500);
	});

	onDestroy(() => {
		if (intervalId !== null) window.clearInterval(intervalId);
	});

	function errorStatus(prefix: string, error: unknown): string {
		return `${prefix}: ${error instanceof Error ? error.message : String(error)}`;
	}

	async function refreshBackend(setStatus = true) {
		try {
			const result = await backendDiagnosticsQuery.refetch();
			if (result.error) throw result.error;
			if (setStatus) status = "Trace refreshed.";
		} catch (error) {
			status = errorStatus("Could not refresh diagnostics", error);
		}
	}

	async function refreshTrace(setStatus = true) {
		snapshot = getDiagnosticsSnapshot();
		await refreshBackend(setStatus);
	}

	async function clearTrace() {
		clearDiagnostics();
		snapshot = getDiagnosticsSnapshot();
		try {
			await clearBackendDiagnostics(client);
			queryClient.setQueryData(queryKeys.backendDiagnostics(), []);
			status = "Trace cleared.";
		} catch (error) {
			status = errorStatus("Could not clear diagnostics", error);
		}
	}

	async function copyReport() {
		try {
			const result = await backendDiagnosticsQuery.refetch();
			if (result.error) throw result.error;
			const events = result.data ?? [];
			const report = createLatencyReport({
				backendEvents: events,
				includeIdentifiers,
			});
			await navigator.clipboard.writeText(report);
			status = includeIdentifiers ? "Report copied." : "Redacted report copied.";
		} catch (error) {
			status = errorStatus("Could not copy diagnostics report", error);
		}
	}
</script>

<FieldGroup>
	<SettingsRow
		title="Enable diagnostics"
		description="Collects local latency timings for release smoke testing. Trace data stays in memory."
	>
		<Checkbox
			checked={settings.debugModeEnabled}
			onCheckedChange={settings.setDebugModeEnabled}
			aria-label="Enable diagnostics"
		/>
	</SettingsRow>

	<SettingsRow
		title="Topology spike"
		description="Opens the Svelte topology benchmark harness with synthetic 4,000-node data."
	>
		<Button
			type="button"
			variant="outline"
			size="sm"
			onclick={() => window.location.assign(TOPOLOGY_SPIKE_URL)}
		>
			<ExternalLink data-icon="inline-start" />
			Open 4k LOD
		</Button>
	</SettingsRow>

	<div class="rounded-md border bg-card p-3">
		<div class="flex flex-wrap items-start justify-between gap-3">
			<div class="min-w-0 space-y-1">
				<div class="text-sm font-medium">Latency report</div>
				<p class="text-xs text-muted-foreground">
					Shows frontend and backend timing summaries. Copy output is redacted by default.
				</p>
			</div>
			<div class="flex flex-wrap items-center gap-2">
				<Button type="button" variant="outline" size="sm" onclick={() => void refreshTrace()}>
					<RefreshCcw data-icon="inline-start" />
					Refresh
				</Button>
				<Button type="button" variant="outline" size="sm" onclick={() => void clearTrace()}>
					<Trash2 data-icon="inline-start" />
					Clear
				</Button>
				<Button type="button" size="sm" onclick={() => void copyReport()}>
					<Copy data-icon="inline-start" />
					Copy
				</Button>
			</div>
		</div>

		<div class="mt-4 flex flex-col gap-4">
			<div class="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
				<span>
					Frontend {snapshot.frontendEvents.length} events, backend {backendEvents.length} events.
				</span>
				<label class="flex items-center gap-2">
					<span>Include identifiers</span>
					<Checkbox
						checked={includeIdentifiers}
						onCheckedChange={(value) => (includeIdentifiers = value)}
						aria-label="Include identifiers in copied latency report"
					/>
				</label>
			</div>

			<div class="space-y-2">
				<div class="text-xs font-medium text-foreground">Frontend timings</div>
				<SimpleTable
					headers={["Name", "Count", "p50", "p95", "Max"]}
					rows={frontendRows}
					empty="No timed frontend spans yet."
				/>
			</div>

			<div class="space-y-2">
				<div class="text-xs font-medium text-foreground">Render counters</div>
				<SimpleTable
					headers={["Name", "Count"]}
					rows={counterRows}
					empty="No render counts yet."
				/>
			</div>

			<div class="space-y-2">
				<div class="text-xs font-medium text-foreground">Backend timings</div>
				<SimpleTable
					headers={["Name", "Count", "p50", "p95", "Max"]}
					rows={backendRows}
					empty="No backend timings yet."
				/>
			</div>

			{#if status}
				<div class="text-xs text-muted-foreground">{status}</div>
			{/if}
		</div>
	</div>
</FieldGroup>

<script lang="ts">
	import { onDestroy } from "svelte";
	import { Copy, Trash2 } from "lucide-svelte";
	import { Button, Switch } from "@/components/ui/svelte";
	import SimpleTable from "@/components/SimpleTable.svelte";
	import {
		clearRecordedInteractions,
		getRecordedInteractions,
		isInteractionRecording,
		startInteractionRecording,
		stopInteractionRecording,
	} from "@/lib/interaction-recorder";
	import SettingsRow from "./SettingsRow.svelte";
	import { interactionTableRows } from "./diagnosticsSettingsModel";

	let recording = $state(isInteractionRecording());
	let interactions = $state(getRecordedInteractions());
	let status = $state("");
	const rows = $derived(interactionTableRows(interactions));
	// Same refresh cadence as the latency report above.
	const intervalId = window.setInterval(() => {
		if (recording) interactions = getRecordedInteractions();
	}, 1_500);

	onDestroy(() => window.clearInterval(intervalId));

	function setRecording(next: boolean) {
		if (next) startInteractionRecording();
		else stopInteractionRecording();
		recording = isInteractionRecording();
		interactions = getRecordedInteractions();
	}

	function clear() {
		clearRecordedInteractions();
		interactions = getRecordedInteractions();
		status = "Interactions cleared.";
	}

	async function copy() {
		try {
			const lines = getRecordedInteractions().map((interaction) => JSON.stringify(interaction));
			await navigator.clipboard.writeText(lines.join("\n"));
			status = `Copied ${lines.length} interactions.`;
		} catch (error) {
			status = `Could not copy interactions: ${error instanceof Error ? error.message : String(error)}`;
		}
	}
</script>

<SettingsRow
	title="Record interactions"
	description="Measures input-to-paint time, dropped frames, and DOM changes for each click, drag, key, and scroll. Adds overhead while on. Data stays in memory; development builds served by Vite also write it to e2e/artifacts/dev-interactions."
>
	<Switch checked={recording} onCheckedChange={setRecording} aria-label="Record interactions" />
</SettingsRow>

<div class="rounded-md border bg-card p-3">
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div class="min-w-0 space-y-1">
			<div class="text-sm font-medium">Interaction report</div>
			<p class="text-xs text-muted-foreground">
				Interactions with dropped frames, worst first. Enable diagnostics to include Tauri command
				timings. Copy outputs JSON lines for <code>bun run perf:interactions</code>.
			</p>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<Button type="button" variant="outline" size="sm" onclick={clear}>
				<Trash2 data-icon="inline-start" />
				Clear
			</Button>
			<Button type="button" size="sm" onclick={() => void copy()}>
				<Copy data-icon="inline-start" />
				Copy
			</Button>
		</div>
	</div>

	<div class="mt-4 flex flex-col gap-2">
		<span class="text-xs text-muted-foreground">{interactions.length} interactions recorded.</span>
		<SimpleTable
			headers={["Time", "Kind", "Target", "Dropped", "Max frame", "To paint", "DOM", "Top source"]}
			{rows}
			empty="No interactions with dropped frames yet."
		/>
		{#if status}
			<div class="text-xs text-muted-foreground">{status}</div>
		{/if}
	</div>
</div>

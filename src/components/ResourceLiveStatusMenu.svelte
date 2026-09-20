<script lang="ts">
	import { ChevronDown, RefreshCw } from "lucide-svelte";
	import { Button, Popover, PopoverContent, PopoverTrigger, Spinner } from "@/components/ui/svelte";
	import { messageFromError } from "@/lib/error-redaction";
	let { onRefresh, disabled = false, status, message, connectionError = null }: {
		onRefresh: () => Promise<void>;
		disabled?: boolean;
		status: string;
		message: string;
		connectionError?: unknown;
	} = $props();
	let open = $state(false);
	let refreshing = $state(false);
	let error = $state<string | null>(null);
	const connectionProblem = $derived(Boolean(connectionError) || status === "reconnecting" || status === "error");
	const label = $derived(
		refreshing ? "Refreshing…"
			: error ? "Refresh failed"
			: disabled ? "Unavailable"
			: connectionProblem ? "Reconnecting"
			: status === "connected" ? "Live"
			: status === "connecting" ? "Connecting" : "Not connected",
	);
	const dotClass = $derived(
		error ? "bg-destructive"
			: disabled ? "bg-muted-foreground"
			: connectionProblem ? "bg-amber-400"
			: status === "connected" ? "bg-emerald-400" : "bg-muted-foreground",
	);
	async function refresh() {
		if (refreshing || disabled) return;
		refreshing = true;
		error = null;
		try { await onRefresh(); } catch (cause) { error = messageFromError(cause); }
		finally { refreshing = false; }
	}
</script>

<Popover bind:open>
	<PopoverTrigger class="inline-flex h-8 shrink-0 items-center gap-2 rounded-md border border-border bg-background/50 px-2.5 text-xs hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring" aria-label="Live updates and refresh options">
		{#if refreshing}<Spinner />{:else}<span class={`size-1.5 rounded-full ${dotClass}`} aria-hidden="true"></span>{/if}
		<span class={error ? "text-destructive" : connectionProblem ? "text-amber-700 dark:text-amber-300" : "text-muted-foreground"} aria-live="polite">{label}</span>
		<ChevronDown class="size-3 text-muted-foreground" aria-hidden="true" />
	</PopoverTrigger>
	<PopoverContent align="end" class="w-72 space-y-3 p-3">
		<p class="text-xs font-medium">Current view</p>
		<p role="status" class="text-xs leading-relaxed text-muted-foreground">{message}</p>
		{#if connectionError}<p class="break-words text-xs text-amber-700 dark:text-amber-300">{messageFromError(connectionError)}</p>{/if}
		{#if error}<p role="alert" class="break-words text-xs text-destructive">Refresh failed: {error}</p>{/if}
		<p class="text-xs leading-relaxed text-muted-foreground">Clear cached data for this view and fetch it again. Filters and selection are preserved.</p>
		<Button type="button" variant="outline" size="sm" class="w-full" disabled={disabled || refreshing} onclick={() => void refresh()}>
			{#if refreshing}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}
			{refreshing ? "Refreshing…" : "Refresh (clear cache)"}
		</Button>
	</PopoverContent>
</Popover>

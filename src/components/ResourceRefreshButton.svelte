<script lang="ts">
	import { RefreshCw } from "lucide-svelte";
	import { Button, Spinner } from "@/components/ui/svelte";
	import { messageFromError } from "@/lib/error-redaction";
	let { onRefresh, disabled = false }: { onRefresh: () => Promise<void>; disabled?: boolean } = $props();
	let refreshing = $state(false);
	let error = $state<string | null>(null);
	async function refresh() {
		if (refreshing || disabled) return;
		refreshing = true;
		error = null;
		try { await onRefresh(); } catch (cause) { error = messageFromError(cause); }
		finally { refreshing = false; }
	}
</script>

<div class="flex flex-wrap items-center justify-end gap-2">
	{#if error}<span role="alert" class="text-xs text-destructive">Refresh failed: {error}</span>{/if}
	<Button type="button" variant="outline" size="sm" disabled={disabled || refreshing} onclick={() => void refresh()} title="Clear cached data for this view and fetch it again. Filters and selection are preserved.">
		{#if refreshing}<Spinner />{:else}<RefreshCw data-icon="inline-start" />{/if}
		{refreshing ? "Refreshing…" : "Refresh (clear cache)"}
	</Button>
</div>

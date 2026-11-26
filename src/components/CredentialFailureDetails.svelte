<script lang="ts">
	import { Check, Copy } from "lucide-svelte";
	import { Button } from "@/components/ui/svelte";
	import { credentialDetails } from "@/lib/credential-failure";
	import type { CredentialFailure } from "@/lib/types";

	let { failure }: { failure: CredentialFailure } = $props();

	let copied = $state(false);
	let copyFailed = $state(false);
	const details = $derived(credentialDetails(failure));

	async function copyCommand() {
		try {
			await navigator.clipboard.writeText(failure.reproduce);
			copied = true;
			copyFailed = false;
			window.setTimeout(() => {
				copied = false;
			}, 1200);
		} catch {
			copyFailed = true;
		}
	}
</script>

{#if failure.pluginMessage}
	<p
		class="rounded bg-destructive/10 px-2 py-1.5 font-mono text-[0.6875rem] leading-relaxed text-destructive [overflow-wrap:anywhere]"
		aria-label="Plugin output"
	>{failure.pluginMessage}</p>
{/if}
<div class="flex items-center gap-2">
	<Button type="button" variant="outline" size="xs" onclick={copyCommand}>
		{#if copied}
			<Check data-icon="inline-start" />
			Copied
		{:else}
			<Copy data-icon="inline-start" />
			Copy terminal command
		{/if}
	</Button>
	{#if copyFailed}
		<span class="text-[0.6875rem] text-muted-foreground">Copy failed</span>
	{/if}
</div>
<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-[0.6875rem]" aria-label="Sign-in details">
	{#each details as [label, value], index (index)}
		<dt class="text-muted-foreground">{label}</dt>
		<dd class="min-w-0 font-mono text-foreground/80 [overflow-wrap:anywhere]">{value}</dd>
	{/each}
</dl>

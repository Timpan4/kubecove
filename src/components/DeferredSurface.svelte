<script lang="ts" generics="T">
	import type { Snippet } from "svelte";
	import { Button } from "@/components/ui/svelte";

	let {
		load,
		label,
		children,
		onRetry = () => window.location.reload(),
	}: {
		load: () => Promise<{ default: T }>;
		label: string;
		children: Snippet<[T]>;
		onRetry?: () => void;
	} = $props();
	const pending = $derived(load());
</script>

{#await pending}
	<p role="status" class="p-4 text-sm text-muted-foreground">Loading {label}…</p>
{:then loaded}
	{@render children(loaded.default)}
{:catch}
	<div role="alert" class="flex flex-col items-start gap-3 p-4 text-sm">
		<p>Could not load {label}.</p>
		<Button variant="outline" onclick={onRetry}>Reload to retry {label}</Button>
	</div>
{/await}

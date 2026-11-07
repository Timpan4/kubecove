<script lang="ts" generics="T">
	import type { Snippet } from "svelte";
	import { Button } from "@/components/ui/svelte";
	import { loadingDelay } from "@/lib/loading-delay.svelte";

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
	let result = $state<{ status: "pending" } | { status: "loaded"; value: T } | { status: "failed" }>({
		status: "pending",
	});
	$effect(() => {
		const pending = load();
		let current = true;
		result = { status: "pending" };
		pending.then(
			(loaded) => { if (current) result = { status: "loaded", value: loaded.default }; },
			() => { if (current) result = { status: "failed" }; },
		);
		return () => { current = false; };
	});
	const loading = loadingDelay(() => result.status === "pending");
</script>

{#if loading.active}
	{#if loading.shown}<p role="status" class="p-4 text-sm text-muted-foreground">Loading {label}…</p>{/if}
{:else if result.status === "loaded"}
	{@render children(result.value)}
{:else if result.status === "failed"}
	<div role="alert" class="flex flex-col items-start gap-3 p-4 text-sm">
		<p>Could not load {label}.</p>
		<Button variant="outline" onclick={onRetry}>Reload to retry {label}</Button>
	</div>
{/if}

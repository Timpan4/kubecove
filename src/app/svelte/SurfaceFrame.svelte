<script lang="ts">
	import type { ComponentType, Snippet, SvelteComponent } from "svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Spinner,
	} from "@/components/ui/svelte";

	type LucideComponent = ComponentType<SvelteComponent>;
	type QueryLike = {
		isPending: boolean;
		isError: boolean;
		error: unknown;
	};

	let {
		icon: Icon,
		title,
		query,
		errorLabel,
		children,
	}: {
		icon: LucideComponent;
		title: string;
		query: QueryLike;
		errorLabel: string;
		children: Snippet;
	} = $props();

	function errorText(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}
</script>

<section class="mx-auto flex w-full max-w-6xl flex-col gap-4 p-4 md:p-6">
	<header class="flex items-center gap-3">
		<Icon class="size-5 text-muted-foreground" />
		<div>
			<h2 class="font-heading text-lg font-semibold">{title}</h2>
			<p class="text-xs text-muted-foreground">Read-only data from existing Tauri commands.</p>
		</div>
	</header>
	{#if query.isPending}
		<div class="flex min-h-48 items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" /> Loading...
		</div>
	{:else if query.isError}
		<Alert variant="destructive">
			<AlertTitle>{errorLabel}</AlertTitle>
			<AlertDescription>{errorText(query.error, errorLabel)}</AlertDescription>
		</Alert>
	{:else}
		{@render children()}
	{/if}
</section>

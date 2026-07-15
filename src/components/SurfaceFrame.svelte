<script lang="ts">
	import type { ComponentType, Snippet, SvelteComponent } from "svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { Spinner } from "@/components/ui/svelte";

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
		wide = false,
		children,
	}: {
		icon: LucideComponent;
		title: string;
		query: QueryLike;
		errorLabel: string;
		wide?: boolean;
		children: Snippet;
	} = $props();

</script>

<section
	class={wide
		? "flex w-full flex-col gap-4 px-3 py-4 md:px-5 md:py-5"
		: "mx-auto flex w-full max-w-7xl flex-col gap-4 px-3 py-4 md:px-4 md:py-5"}
>
	<header class="flex items-center gap-3">
		<Icon class="size-5 text-muted-foreground" />
		<div>
			<h2 class="font-heading text-lg font-semibold">{title}</h2>
		</div>
	</header>
	{#if query.isPending}
		<div class="flex min-h-48 items-center justify-center gap-2 text-sm text-muted-foreground">
			<Spinner class="size-4" /> Loading...
		</div>
	{:else if query.isError}
		<FriendlyError
			error={query.error}
			context={{ operation: "surfaceLoad", fallbackTitle: errorLabel }}
		/>
	{:else}
		{@render children()}
	{/if}
</section>

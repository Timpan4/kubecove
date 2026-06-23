<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { SearchIcon } from "lucide-svelte";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<div
	data-slot="command-input-wrapper"
	class="flex h-10 items-center gap-2 border-b px-3"
	data-cmdk-input-wrapper=""
>
	<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
	<CommandPrimitive.Input
		data-slot="command-input"
		class={cn(
			"flex h-9 w-full rounded-md bg-transparent py-2 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50",
			className,
		)}
		{...rest}
	/>
</div>

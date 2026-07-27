<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import Separator from "./Separator.svelte";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<Separator
	data-slot="select-separator"
	class={cnfast("pointer-events-none -mx-1 my-1 h-px bg-border/50", className)}
	{...rest}
/>

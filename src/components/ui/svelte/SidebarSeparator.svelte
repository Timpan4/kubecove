<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import Separator from "./Separator.svelte";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<Separator
	data-slot="sidebar-separator"
	data-sidebar="separator"
	class={cnfast("mx-2 w-auto bg-sidebar-border", className)}
	{...rest}
/>

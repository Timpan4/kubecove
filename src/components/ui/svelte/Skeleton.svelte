<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<div data-slot="skeleton" class={cnfast("animate-pulse rounded-md bg-muted", className)} {...rest}></div>

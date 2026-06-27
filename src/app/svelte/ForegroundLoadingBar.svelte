<script lang="ts">
	import { foregroundLoading } from "@/lib/foreground-loading-svelte";
	import { cnfast } from "@/lib/utils";

	const active = $derived($foregroundLoading > 0);
</script>

<div
	aria-hidden="true"
	class={cnfast(
		"h-0.5 overflow-hidden bg-transparent opacity-0 transition-opacity",
		active && "opacity-100",
	)}
>
	<div class="foreground-loading-bar h-full w-1/3 rounded-full bg-primary"></div>
</div>

<script lang="ts">
	import { tick } from "svelte";
	import { foregroundLoading } from "@/lib/foreground-loading-svelte";
	import { cnfast } from "@/lib/utils";

	const active = $derived($foregroundLoading > 0);
	let animating = $state(false);
	let host: HTMLDivElement;
	$effect(() => {
		if (active) {
			animating = true;
			return;
		}
		let cancelled = false;
		void tick().then(async () => {
			if (cancelled) return;
			// Wait for the actual opacity transition, including reduced/zero motion.
			await Promise.allSettled(host.getAnimations().map((animation) => animation.finished));
			if (!cancelled) animating = false;
		});
		return () => { cancelled = true; };
	});
</script>

<div
	aria-hidden="true"
	bind:this={host}
	class={cnfast(
		"h-0.5 overflow-hidden bg-transparent opacity-0 transition-opacity",
		active && "opacity-100",
	)}
>
	<div class={cnfast("h-full w-1/3 rounded-full bg-primary", animating && "foreground-loading-bar")}></div>
</div>

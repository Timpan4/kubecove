<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let {
		children,
		delayDuration = 0,
		skipDelayDuration = 0,
		...rest
	}: UiProps & { delayDuration?: number; skipDelayDuration?: number } = $props();
</script>

<TooltipPrimitive.Provider {delayDuration} {skipDelayDuration} {...rest}>
	{@render children?.()}
</TooltipPrimitive.Provider>

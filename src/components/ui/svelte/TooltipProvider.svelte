<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, delayDuration = 0, ...rest }: UiProps & { delayDuration?: number } =
		$props();
</script>

<TooltipPrimitive.Provider {delayDuration} {...rest}>
	{@render children?.()}
</TooltipPrimitive.Provider>

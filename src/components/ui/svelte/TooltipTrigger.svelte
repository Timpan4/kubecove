<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<TooltipPrimitive.Trigger data-slot="tooltip-trigger" {...rest}>
	{@render children?.()}
</TooltipPrimitive.Trigger>

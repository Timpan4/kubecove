<script lang="ts">
	import { Tooltip as TooltipPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<TooltipPrimitive.Root {...rest}>
	{@render children?.()}
</TooltipPrimitive.Root>

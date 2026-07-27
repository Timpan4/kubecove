<script lang="ts">
	import { Popover as PopoverPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<PopoverPrimitive.Trigger data-slot="popover-trigger" {...rest}>
	{@render children?.()}
</PopoverPrimitive.Trigger>

<script lang="ts">
	import { Popover as PopoverPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, open = $bindable(), ...rest }: UiProps & { open?: boolean } = $props();
</script>

<PopoverPrimitive.Root bind:open {...rest}>
	{@render children?.()}
</PopoverPrimitive.Root>

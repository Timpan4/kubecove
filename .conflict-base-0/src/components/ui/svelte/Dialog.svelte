<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Root {...rest}>
	{@render children?.()}
</DialogPrimitive.Root>

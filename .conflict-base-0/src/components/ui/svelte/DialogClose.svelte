<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Close data-slot="dialog-close" {...rest}>
	{@render children?.()}
</DialogPrimitive.Close>

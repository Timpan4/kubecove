<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Trigger data-slot="dialog-trigger" {...rest}>
	{@render children?.()}
</DialogPrimitive.Trigger>

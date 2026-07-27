<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Portal {...rest}>
	{@render children?.()}
</DialogPrimitive.Portal>

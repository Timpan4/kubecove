<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let {
		type = "single",
		children,
		...rest
	}: UiProps & { type?: "single" | "multiple" } = $props();
</script>

<SelectPrimitive.Root {type} {...rest}>
	{@render children?.()}
</SelectPrimitive.Root>

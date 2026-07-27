<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<SelectPrimitive.GroupHeading
	data-slot="select-label"
	class={cnfast("px-2 py-1.5 text-xs text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</SelectPrimitive.GroupHeading>

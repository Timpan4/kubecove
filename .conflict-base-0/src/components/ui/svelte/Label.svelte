<script lang="ts">
	import { Label as LabelPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<LabelPrimitive.Root
	data-slot="label"
	class={cnfast(
		"flex items-center gap-2 text-xs/relaxed leading-none font-medium select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</LabelPrimitive.Root>

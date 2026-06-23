<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div
	data-slot="field-content"
	class={cn(
		"group/field-content flex flex-1 flex-col gap-0.5 leading-snug",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</div>

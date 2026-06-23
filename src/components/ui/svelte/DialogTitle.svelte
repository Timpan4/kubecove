<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Title
	data-slot="dialog-title"
	class={cn("font-heading text-sm font-medium text-foreground", className)}
	{...rest}
>
	{@render children?.()}
</DialogPrimitive.Title>

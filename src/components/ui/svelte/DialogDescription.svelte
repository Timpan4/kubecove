<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Description
	data-slot="dialog-description"
	class={cn("text-xs/relaxed text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</DialogPrimitive.Description>

<script lang="ts">
	import { Dialog as DialogPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import DialogOverlay from "./DialogOverlay.svelte";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<DialogPrimitive.Portal>
	<DialogOverlay />
	<DialogPrimitive.Content
		data-slot="dialog-content"
		class={cnfast(
			"fixed left-1/2 top-1/2 z-dialog grid w-full max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 rounded-lg border bg-surface-2 p-6 text-popover-foreground shadow-2xl duration-100 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95",
			className,
		)}
		{...rest}
	>
		{@render children?.()}
	</DialogPrimitive.Content>
</DialogPrimitive.Portal>

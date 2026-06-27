<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import SelectScrollDownButton from "./SelectScrollDownButton.svelte";
	import SelectScrollUpButton from "./SelectScrollUpButton.svelte";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		children,
		side = "bottom",
		align = "center",
		sideOffset = 4,
		...rest
	}: UiProps & {
		side?: "top" | "right" | "bottom" | "left";
		align?: "start" | "center" | "end";
		sideOffset?: number;
	} = $props();
</script>

<SelectPrimitive.Portal>
	<SelectPrimitive.Content
		data-slot="select-content"
		class={cnfast(
			"relative max-h-(--bits-select-content-available-height) min-w-32 origin-(--bits-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-lg bg-popover/70 text-popover-foreground shadow-md ring-1 ring-foreground/10 duration-100 before:pointer-events-none before:absolute before:inset-0 before:-z-1 before:rounded-[inherit] before:backdrop-blur-2xl before:backdrop-saturate-150 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95 **:data-[slot$=-item]:focus:bg-foreground/10 **:data-[slot$=-item]:data-highlighted:bg-foreground/10 **:data-[slot$=-separator]:bg-foreground/5",
			className,
		)}
		{side}
		{align}
		{sideOffset}
		{...rest}
	>
		<SelectScrollUpButton />
		<SelectPrimitive.Viewport data-slot="select-viewport">
			{@render children?.()}
		</SelectPrimitive.Viewport>
		<SelectScrollDownButton />
	</SelectPrimitive.Content>
</SelectPrimitive.Portal>

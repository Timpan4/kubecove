<script lang="ts">
	import Button from "./Button.svelte";
	import { cnfast } from "@/lib/utils";
	import type { ButtonSize, ButtonVariant } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		type = "button",
		variant = "ghost",
		size = "xs",
		children,
		...rest
	}: UiProps & {
		type?: "button" | "submit" | "reset";
		variant?: ButtonVariant;
		size?: Extract<ButtonSize, "xs" | "sm" | "icon-xs" | "icon-sm">;
	} = $props();
</script>

<Button
	{type}
	{variant}
	{size}
	data-size={size}
	class={cnfast(
		"flex items-center gap-2 rounded-md text-xs/relaxed shadow-none",
		size === "xs" &&
			"h-5 gap-1 rounded-[calc(var(--radius-sm)-2px)] px-1 [&>svg:not([class*='size-'])]:size-3",
		size === "sm" && "gap-1",
		size === "icon-xs" && "size-6 p-0 has-[>svg]:p-0",
		size === "icon-sm" && "size-7 p-0 has-[>svg]:p-0",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</Button>

<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		size = "default",
		variant = "default",
		isActive = false,
		type = "button",
		children,
		...rest
	}: UiProps & {
		size?: "default" | "sm" | "lg";
		variant?: "default" | "outline";
		isActive?: boolean;
		type?: "button" | "submit" | "reset";
	} = $props();
</script>

<button
	{type}
	data-slot="sidebar-menu-button"
	data-sidebar="menu-button"
	data-size={size}
	data-active={isActive}
	data-variant={variant}
	class={cnfast(
		"peer/menu-button group/menu-button flex w-full items-center gap-2 overflow-hidden rounded-[calc(var(--radius-sm)+2px)] p-2 text-left text-xs ring-sidebar-ring outline-hidden transition-[width,height,padding] hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 active:bg-sidebar-accent active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 data-active:bg-sidebar-accent data-active:font-medium data-active:text-sidebar-accent-foreground [&_svg]:size-4 [&_svg]:shrink-0 [&>span:last-child]:truncate",
		size === "default" && "h-8",
		size === "sm" && "h-7",
		size === "lg" && "h-12",
		variant === "outline" &&
			"bg-background shadow-[0_0_0_1px_var(--sidebar-border)] hover:shadow-[0_0_0_1px_var(--sidebar-accent)]",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</button>

<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		checked = false,
		onCheckedChange,
		disabled = false,
		type = "button",
		...rest
	}: UiProps & {
		checked?: boolean;
		onCheckedChange?: (checked: boolean) => void;
		disabled?: boolean;
		type?: "button" | "submit" | "reset";
	} = $props();

	function toggle() {
		if (disabled) return;
		onCheckedChange?.(!checked);
	}
</script>

<button
	data-slot="switch"
	data-checked={checked ? "" : undefined}
	role="switch"
	aria-checked={checked}
	class={cnfast(
		"relative inline-flex h-6 w-10 shrink-0 cursor-pointer items-center rounded-full border border-input bg-input/40 p-0.5 transition-colors outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50 data-checked:border-primary data-checked:bg-primary",
		className,
	)}
	{disabled}
	{type}
	onclick={toggle}
	{...rest}
>
	<span
		data-slot="switch-thumb"
		class="block size-4 rounded-full bg-background shadow-sm transition-transform data-checked:translate-x-4"
		data-checked={checked ? "" : undefined}
	></span>
</button>

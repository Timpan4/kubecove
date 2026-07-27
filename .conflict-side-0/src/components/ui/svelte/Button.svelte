<script lang="ts">
	import { buttonClass, type ButtonSize, type ButtonVariant } from "./classes";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		variant = "default",
		size = "default",
		type = "button",
		children,
		...rest
	}: UiProps & {
		variant?: ButtonVariant;
		size?: ButtonSize;
		type?: "button" | "submit" | "reset";
	} = $props();
</script>

<button
	data-slot="button"
	data-variant={variant}
	data-size={size}
	class={buttonClass({ variant, size, className })}
	{type}
	{...rest}
>
	{@render children?.()}
</button>

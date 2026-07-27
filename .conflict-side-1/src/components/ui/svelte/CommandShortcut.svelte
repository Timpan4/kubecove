<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<span
	data-slot="command-shortcut"
	class={cnfast("ml-auto text-[10px] tracking-widest text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</span>

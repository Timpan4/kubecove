<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<tr
	data-slot="table-row"
	class={cnfast(
		"border-b transition-colors hover:bg-muted/50 has-aria-expanded:bg-muted/50 data-[state=selected]:bg-muted",
		className,
	)}
	{...rest}
>
	{@render children?.()}
</tr>

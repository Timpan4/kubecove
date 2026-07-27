<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<tfoot
	data-slot="table-footer"
	class={cnfast("border-t bg-muted/50 font-medium [&>tr]:last:border-b-0", className)}
	{...rest}
>
	{@render children?.()}
</tfoot>

<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<tbody
	data-slot="table-body"
	class={cn("[&_tr:last-child]:border-0", className)}
	{...rest}
>
	{@render children?.()}
</tbody>

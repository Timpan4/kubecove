<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<tbody
	data-slot="table-body"
	class={cnfast("[&_tr:last-child]:border-0", className)}
	{...rest}
>
	{@render children?.()}
</tbody>

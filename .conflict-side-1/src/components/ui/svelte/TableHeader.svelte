<script lang="ts">
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<thead data-slot="table-header" class={cnfast("[&_tr]:border-b", className)} {...rest}>
	{@render children?.()}
</thead>

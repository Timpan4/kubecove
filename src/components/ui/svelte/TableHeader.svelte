<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<thead data-slot="table-header" class={cn("[&_tr]:border-b", className)} {...rest}>
	{@render children?.()}
</thead>

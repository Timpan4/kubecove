<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="dialog-header" class={cn("flex flex-col gap-1.5", className)} {...rest}>
	{@render children?.()}
</div>

<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sheet-footer" class={cn("mt-auto flex flex-col gap-2 p-6", className)} {...rest}>
	{@render children?.()}
</div>

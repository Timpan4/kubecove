<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<caption
	data-slot="table-caption"
	class={cn("mt-4 text-xs text-muted-foreground", className)}
	{...rest}
>
	{@render children?.()}
</caption>

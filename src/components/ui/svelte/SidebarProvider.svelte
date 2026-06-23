<script lang="ts">
	import { cn } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", children, ...rest }: UiProps = $props();
</script>

<div data-slot="sidebar-provider" class={cn("flex min-h-svh w-full", className)} {...rest}>
	{@render children?.()}
</div>

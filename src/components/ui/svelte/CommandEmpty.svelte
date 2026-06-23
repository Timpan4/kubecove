<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import type { UiProps } from "./types";

	let { children, ...rest }: UiProps = $props();
</script>

<CommandPrimitive.Empty
	data-slot="command-empty"
	class="py-6 text-center text-xs text-muted-foreground"
	{...rest}
>
	{@render children?.()}
</CommandPrimitive.Empty>

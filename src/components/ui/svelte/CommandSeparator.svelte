<script lang="ts">
	import { Command as CommandPrimitive } from "bits-ui";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let { class: className = "", ...rest }: UiProps = $props();
</script>

<CommandPrimitive.Separator
	data-slot="command-separator"
	class={cnfast("-mx-1 h-px bg-border", className)}
	{...rest}
/>

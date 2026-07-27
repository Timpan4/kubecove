<script lang="ts">
	import Dialog from "./Dialog.svelte";
	import DialogContent from "./DialogContent.svelte";
	import DialogDescription from "./DialogDescription.svelte";
	import DialogHeader from "./DialogHeader.svelte";
	import DialogTitle from "./DialogTitle.svelte";
	import Command from "./Command.svelte";
	import { cnfast } from "@/lib/utils";
	import type { UiProps } from "./types";

	let {
		class: className = "",
		title = "Command Palette",
		description = "Search for a command to run...",
		commandProps = {},
		children,
		...rest
	}: UiProps & { title?: string; description?: string; commandProps?: Record<string, unknown> } = $props();
</script>

<Dialog {...rest}>
	<DialogContent class={cnfast("top-[20%] translate-y-0 overflow-hidden p-0", className)}>
		<DialogHeader class="sr-only">
			<DialogTitle>{title}</DialogTitle>
			<DialogDescription>{description}</DialogDescription>
		</DialogHeader>
		<Command
			class="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:text-muted-foreground [&_[cmdk-group]]:px-2 [&_[data-cmdk-input-wrapper]_svg]:h-4 [&_[data-cmdk-input-wrapper]_svg]:w-4 [&_[cmdk-item]]:px-2 [&_[cmdk-item]]:py-2 [&_[cmdk-item]_svg]:h-4 [&_[cmdk-item]_svg]:w-4"
			{...commandProps}
		>
			{@render children?.()}
		</Command>
	</DialogContent>
</Dialog>

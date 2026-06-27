<script lang="ts">
	import svelteLogoUrl from "./svelte-logo.svg";
	import { Button } from "@/components/ui/svelte";
	import { queueUiRuntimeSettingsFocus } from "@/lib/ui-runtime";

	let {
		onOpenSettings,
	}: { onOpenSettings: () => void } = $props();
</script>

<Button
	type="button"
	variant="outline"
	size="sm"
	class="mr-2 h-8 gap-1.5 px-2 text-xs text-muted-foreground [-webkit-app-region:no-drag]"
	aria-label="Open Settings"
	data-ui-runtime="svelte"
	onclick={() => {
		queueUiRuntimeSettingsFocus();
		onOpenSettings();
	}}
>
	<img src={svelteLogoUrl} alt="" class="size-3.5" aria-hidden="true" />
	<span>Svelte</span>
</Button>

<script lang="ts">
	import svelteLogoUrl from "./svelte-logo.svg";
	import { Button } from "@/components/ui/svelte";
	import { isBrowserDevMockMode } from "@/lib/tauri";
	import { queueUiRuntimeSettingsFocus } from "@/lib/ui-runtime";

	let {
		onOpenSettings,
	}: { onOpenSettings: () => void } = $props();

	const mockMode = isBrowserDevMockMode();
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
	{#if mockMode}
		<span class="rounded border border-amber-500/40 px-1 text-[0.65rem] font-semibold uppercase text-amber-700 dark:text-amber-300">
			Mock
		</span>
	{/if}
</Button>

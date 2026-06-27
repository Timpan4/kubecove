<script lang="ts">
	import reactLogoUrl from "./react-logo.svg";
	import svelteLogoUrl from "./svelte-logo.svg";
	import { Button } from "@/components/ui/svelte";
	import {
		queueUiRuntimeSettingsFocus,
		uiRuntimeModeLabel,
		type UiRuntimeMode,
	} from "@/lib/ui-runtime";

	let {
		mode,
		onOpenSettings,
	}: { mode: UiRuntimeMode; onOpenSettings: () => void } = $props();

	const label = $derived(uiRuntimeModeLabel(mode));
	const logoUrl = $derived(mode === "svelte" ? svelteLogoUrl : reactLogoUrl);
</script>

<Button
	type="button"
	variant="outline"
	size="sm"
	class="mr-2 h-8 gap-1.5 px-2 text-xs text-muted-foreground [-webkit-app-region:no-drag]"
	aria-label={`Open Settings for ${label} UI mode`}
	data-ui-runtime={mode}
	onclick={() => {
		queueUiRuntimeSettingsFocus();
		onOpenSettings();
	}}
>
	<img src={logoUrl} alt="" class="size-3.5" aria-hidden="true" />
	<span>{label}</span>
</Button>

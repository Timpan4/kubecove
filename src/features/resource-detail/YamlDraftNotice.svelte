<script lang="ts">
	import { Alert, AlertDescription } from "@/components/ui/svelte";
	import { yamlDraftNotice } from "./yamlDraftGuard.svelte";
</script>

{#if yamlDraftNotice.current}
	<Alert class="shrink-0 rounded-none border-x-0 border-t-0">
		<AlertDescription>{yamlDraftNotice.current}</AlertDescription>
	</Alert>
{/if}

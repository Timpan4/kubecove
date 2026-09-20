<script lang="ts">
	import type { ArgoApplicationInspector } from "@/lib/types";
	import { argoOperationProgress } from "./argo-operation-progress";
	let { inspector }: { inspector: ArgoApplicationInspector } = $props();
	const progress = $derived(argoOperationProgress(inspector));
</script>

{#if progress.phase || progress.refreshing}
	<section class="rounded-md border p-3 text-xs" aria-label="Argo CD operation progress">
		<div role="status" aria-live="polite" class="font-medium">
			{progress.refreshing ? "Refreshing Application" : `Operation: ${progress.phase}`}
		</div>
		{#if progress.message}<p class="mt-1 break-words text-muted-foreground">{progress.message}</p>{/if}
		{#if progress.resources.length}
			<div class="mt-2 overflow-x-auto">
				<table class="w-full text-left">
					<thead><tr><th class="p-1">Resource</th><th class="p-1">Phase</th><th class="p-1">Message</th></tr></thead>
					<tbody>{#each progress.resources as resource}
						<tr class="border-t"><td class="p-1">{resource.kind} {resource.namespace ? resource.namespace + '/' : ''}{resource.name}</td><td class="p-1">{resource.phase}</td><td class="break-words p-1">{resource.message || "No message reported"}</td></tr>
					{/each}</tbody>
				</table>
			</div>
		{/if}
	</section>
{/if}

<script lang="ts">
	import { AlertTriangle } from "lucide-svelte";
	import { Alert, AlertDescription, AlertTitle } from "@/components/ui/svelte";
	import type { ResourceScopeFailure } from "@/lib/types";
	let { failures }: { failures: ResourceScopeFailure[] } = $props();
</script>

{#if failures.length > 0}
	<Alert class="shrink-0">
		<AlertTriangle class="size-4" />
		<AlertTitle>Some resources are unavailable</AlertTitle>
		<AlertDescription>
			<p>Showing resources this account can read. Unavailable resources are excluded from totals.</p>
			<ul class="mt-2 list-inside list-disc">
				{#each failures as failure}
					<li>{failure.request.kind ?? failure.request.resourceKind?.kind} in {failure.clusterContext}, {failure.request.namespace ?? "all namespaces"}: {failure.error.message}</li>
				{/each}
			</ul>
		</AlertDescription>
	</Alert>
{/if}

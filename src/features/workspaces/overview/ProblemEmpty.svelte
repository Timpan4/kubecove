<script lang="ts">
	import { CardContent, Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/svelte";
	import type { OverviewView } from "./view";

	let { view }: { view: OverviewView } = $props();
</script>

{#if view.hasData}
	<CardContent>
		<Empty class="border-0 p-4">
			<EmptyHeader>
				<EmptyTitle>No failing workloads</EmptyTitle>
				<EmptyDescription>
					{#if view.partial}
						None in the part of the scope that loaded. Resource Health lists what did not load.
					{:else}
						No loaded resource is degraded or needs attention. {view.health.unknown} unknown and
						{view.health.notEvaluated} not evaluated resources carry no health signal.
					{/if}
				</EmptyDescription>
			</EmptyHeader>
		</Empty>
	</CardContent>
{/if}

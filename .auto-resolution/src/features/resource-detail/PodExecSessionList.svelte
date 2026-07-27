<script lang="ts">
	import { Square } from "lucide-svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
	} from "@/components/ui/svelte";
	import { podExecCommandText } from "@/features/live-sessions";
	import type { PodExecSessionSummary } from "@/lib/types";

	let {
		sessions,
		fetching,
		showKubeconfigSourceLabels,
		stoppingId,
		onStop,
	}: {
		sessions: PodExecSessionSummary[];
		fetching: boolean;
		showKubeconfigSourceLabels: boolean;
		stoppingId: string | null;
		onStop: (sessionId: string) => void;
	} = $props();
</script>

<div class="flex items-center justify-between gap-2">
	<div class="text-xs font-semibold uppercase text-muted-foreground">Active exec sessions</div>
	{#if fetching}<Spinner />{/if}
</div>
{#if sessions.length === 0}
	<Empty class="min-h-24 border border-dashed">
		<EmptyHeader>
			<EmptyTitle>No exec sessions</EmptyTitle>
			<EmptyDescription>Start guarded exec for this Pod.</EmptyDescription>
		</EmptyHeader>
	</Empty>
{:else}
	<div class="flex flex-col gap-2">
		{#each sessions as item (item.id)}
			<div class="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-background p-3">
				<div class="min-w-0">
					<div class="truncate font-mono text-xs">{podExecCommandText(item.command)}</div>
					<div class="text-[11px] text-muted-foreground">
						{item.container ? `Container ${item.container}` : "Default container"}
					</div>
					{#if showKubeconfigSourceLabels && item.kubeconfigSourceLabel}
						<div class="truncate text-[11px] text-muted-foreground">
							{item.kubeconfigSourceLabel}
						</div>
					{/if}
				</div>
				<div class="flex items-center gap-2">
					<Badge variant={item.status === "error" ? "destructive" : "outline"}>{item.status}</Badge>
					<Button
						variant="outline"
						size="sm"
						onclick={() => onStop(item.id)}
						disabled={stoppingId === item.id}
					>
						{#if stoppingId === item.id}
							<Spinner data-icon="inline-start" />
						{:else}
							<Square data-icon="inline-start" />
						{/if}
						Stop
					</Button>
				</div>
			</div>
		{/each}
	</div>
{/if}

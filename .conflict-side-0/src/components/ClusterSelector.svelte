<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Button,
		Select,
		SelectContent,
		SelectGroup,
		SelectItem,
		SelectTrigger,
		SelectValue,
	} from "@/components/ui/svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import { settingsStore } from "@/lib/settings-store";
	import { createTauriClient, listKubeContexts } from "@/lib/tauri";
	import type { ClusterContext } from "@/lib/types";

	let {
		value,
		onClusterChange,
	}: {
		value?: string;
		onClusterChange: (cluster: string) => void;
	} = $props();

	const client = createTauriClient();
	const kubeconfigSourceKey = $derived($settingsStore.kubeconfigSourceKey);
	const contextsQuery = createQuery<ClusterContext[]>(() => ({
		queryKey: queryKeys.kubeContexts(kubeconfigSourceKey),
		queryFn: () => listKubeContexts(client, kubeconfigSourceKey),
	}));

	const contexts = $derived(contextsQuery.data ?? []);
	const preferredContextName = $derived(
		contexts.find((context) => context.isCurrent)?.name ?? contexts[0]?.name ?? "",
	);
	const selectedValue = $derived(value ?? preferredContextName);

	$effect(() => {
		if (!preferredContextName) return;
		const selectedStillExists = contexts.some((context) => context.name === value);
		if (!value || !selectedStillExists) onClusterChange(preferredContextName);
	});
</script>

{#if contextsQuery.isPending}
	<div class="flex items-center gap-2 text-xs text-muted-foreground">
		Loading contexts...
	</div>
{:else if contextsQuery.isError}
	<div class="flex flex-col gap-2">
		<FriendlyError
			error={contextsQuery.error}
			context={{ operation: "contextLoad", fallbackTitle: "Failed to load contexts" }}
		/>
		<Button type="button" variant="outline" size="sm" onclick={() => void contextsQuery.refetch()}>
			Retry
		</Button>
	</div>
{:else if contexts.length === 0}
	<div class="text-xs text-muted-foreground">No contexts found</div>
{:else}
	<div class="flex flex-row items-center gap-2">
		<span
			id="cluster-select-label"
			class="whitespace-nowrap text-xs font-bold uppercase tracking-wide text-muted-foreground"
		>
			Cluster Context:
		</span>
		<Select value={selectedValue} onValueChange={onClusterChange}>
			<SelectTrigger
				id="cluster-select"
				aria-labelledby="cluster-select-label"
				class="h-8 min-w-40 bg-background/50 text-xs"
			>
				<SelectValue placeholder="Select a context..." />
			</SelectTrigger>
			<SelectContent>
				<SelectGroup>
					{#each contexts as context (context.name)}
						<SelectItem value={context.name}>{context.name}</SelectItem>
					{/each}
				</SelectGroup>
			</SelectContent>
		</Select>
	</div>
{/if}

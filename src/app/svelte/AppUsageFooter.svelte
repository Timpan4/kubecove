<script lang="ts">
	import { createQuery } from "@tanstack/svelte-query";
	import { Activity, CircleAlert } from "lucide-svelte";
	import {
		Popover,
		PopoverContent,
		PopoverTrigger,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
		buttonClass,
	} from "@/components/ui/svelte";
	import { queryKeys } from "@/lib/queryKeys";
	import { createTauriClient, getAppUsageMetrics } from "@/lib/tauri";
	import {
		flattenUsageMetricsBreakdown,
		formatCpuPercent,
		formatMemoryBytes,
		formatProcessCount,
		formatUsageMetrics,
	} from "@/lib/usage-metrics";
	import { settingsStore } from "@/lib/settings-store";

	const client = createTauriClient();
	let open = $state(false);
	const visible = $derived($settingsStore.showUsageFooter);

	const usageQuery = createQuery(() => ({
		queryKey: queryKeys.appUsageMetrics(),
		queryFn: () => getAppUsageMetrics(client),
		enabled: visible,
		placeholderData: (previousData) => previousData,
		refetchInterval: visible ? 2_000 : false,
	}));

	const metrics = $derived(usageQuery.data);
	const rows = $derived(flattenUsageMetricsBreakdown(metrics?.breakdown ?? []));
	const label = $derived(
		usageQuery.isError
			? "Usage metrics unavailable"
			: metrics
				? formatUsageMetrics(metrics)
				: "Loading usage metrics...",
	);

	$effect(() => {
		if (!visible) open = false;
	});
</script>

{#if visible}
	<footer class="relative flex h-7 shrink-0 items-center justify-end gap-2 border-t bg-sidebar px-4 text-xs text-muted-foreground">
		{#if usageQuery.isError || !metrics}
			<span class="inline-flex min-w-0 items-center gap-1.5">
				{#if usageQuery.isError}
					<CircleAlert class="size-3.5 shrink-0" />
				{:else}
					<Activity class="size-3.5 shrink-0" />
				{/if}
				<span class="truncate">{label}</span>
			</span>
		{:else}
			<Popover bind:open>
				<PopoverTrigger
				type="button"
				class={buttonClass({
					variant: "ghost",
					size: "sm",
					className: "h-6 min-w-0 px-2 text-xs font-normal text-muted-foreground hover:text-foreground",
				})}
				aria-label="Show app process tree"
				aria-expanded={open}
			>
				<Activity data-icon="inline-start" class="size-3.5 shrink-0" />
				<span class="truncate">{label}</span>
				</PopoverTrigger>
				<PopoverContent side="top" align="end" class="w-[31rem] max-w-[calc(100vw-2rem)] p-2">
				<div class="px-1 pb-1 text-xs font-semibold uppercase text-muted-foreground">
					App process tree
				</div>
				<div class="max-h-[min(58vh,28rem)] overflow-auto">
					<Table class="border-separate border-spacing-x-0 border-spacing-y-0.5 text-xs">
						<TableHeader>
							<TableRow class="border-0 text-xs font-semibold uppercase text-muted-foreground hover:bg-transparent">
								<TableHead class="h-auto px-1 pb-1 text-left">Process</TableHead>
								<TableHead class="h-auto px-1 pb-1 text-right">CPU</TableHead>
								<TableHead class="h-auto px-1 pb-1 text-right">Memory</TableHead>
								<TableHead class="h-auto px-1 pb-1 text-right">Count</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{#each rows as row (`${row.depth}:${row.item.label}`)}
								<TableRow
									class={row.depth > 0
										? "border-0 text-muted-foreground hover:bg-transparent"
										: "border-0 text-foreground hover:bg-transparent"}
								>
									<TableCell class="min-w-0 py-1 pr-3 align-top">
										<div class="min-w-0" style={`padding-left: ${row.depth * 0.75}rem`}>
											<div class={row.depth > 0 ? "truncate text-muted-foreground" : "truncate font-medium"}>
												{row.item.label}
											</div>
											{#if row.depth === 0}
												<div class="truncate text-xs text-muted-foreground">
													{row.item.description}
												</div>
											{/if}
										</div>
									</TableCell>
									<TableCell class="whitespace-nowrap px-1 py-1 text-right align-top tabular-nums">
										CPU {formatCpuPercent(row.item.cpuPercent)}
									</TableCell>
									<TableCell class="whitespace-nowrap px-1 py-1 text-right align-top tabular-nums">
										{formatMemoryBytes(row.item.memoryBytes)}
									</TableCell>
									<TableCell class="whitespace-nowrap py-1 pl-1 text-right align-top tabular-nums">
										{formatProcessCount(row.item.processCount)}
									</TableCell>
								</TableRow>
							{/each}
						</TableBody>
					</Table>
				</div>
				</PopoverContent>
			</Popover>
		{/if}
	</footer>
{/if}

<script lang="ts">
	import { AlertTriangle, ExternalLink, RotateCcw } from "lucide-svelte";
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Badge,
		Button,
		Card,
		CardContent,
		CardDescription,
		CardHeader,
		CardTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
	} from "@/components/ui/svelte";
	import type { IncidentFilter } from "@/features/incidents/helpers";
	import {
		incidentResourcesHealthFilter as incidentResourcesHealthFilterFor,
		incidentScopeLabel,
		incidentSeverityLabel,
		incidentSignalSummary,
		incidentWarningSummary,
	} from "./incidentSurfaceModel";
	import { treeNodeForResource } from "./workspaceShellModel";
	import StatGrid from "./StatGrid.svelte";
	import SurfaceFrame from "./SurfaceFrame.svelte";

	let {
		workspace,
		incidentsQuery,
		incidentCounts,
		incidentFilter = $bindable("all" as IncidentFilter),
		incidentFilterOptions,
		incidentGroups,
		onOpenResources,
		onResourceSelect,
	} = $props();
</script>

<SurfaceFrame icon={AlertTriangle} title="Incident Cockpit" query={incidentsQuery} errorLabel="Incidents unavailable">
		<div class="flex flex-wrap items-start justify-between gap-3 border-b pb-3">
			<div class="min-w-0 text-xs text-muted-foreground">
				<div class="truncate">{workspace.name}</div>
				<div class="truncate">
					{workspace.scope.namespaces.length === 0 ? "scanning all namespaces" : `scanning ${workspace.scope.namespaces.join(", ")}`}
				</div>
			</div>
			<div class="flex flex-wrap gap-2">
				<Button
					type="button"
					variant="outline"
					size="sm"
					disabled={incidentsQuery.isFetching}
					onclick={() => incidentsQuery.refetch()}
				>
					{#if incidentsQuery.isFetching}
						<Spinner data-icon="inline-start" />
					{:else}
						<RotateCcw data-icon="inline-start" />
					{/if}
					Refresh
				</Button>
				<Button
					type="button"
					variant="outline"
					size="sm"
					onclick={() => onOpenResources(undefined, "", "", incidentResourcesHealthFilterFor(incidentFilter))}
				>
					<ExternalLink data-icon="inline-start" />
					Resources
				</Button>
			</div>
		</div>
		{#if incidentsQuery.data?.warnings.length}
			<Alert>
				<AlertTitle>Partial incident data</AlertTitle>
				<AlertDescription>{incidentsQuery.data.warnings.join(" ")}</AlertDescription>
			</Alert>
		{/if}
		<StatGrid
			stats={[
				["All", incidentCounts.total],
				["Degraded", incidentCounts.degraded],
				["Attention", incidentCounts.attention],
				["Restarted", incidentCounts.restarted],
				["Warnings", incidentCounts.warning],
			]}
		/>
		<div class="flex flex-wrap gap-2">
			{#each incidentFilterOptions as option}
				<Button
					type="button"
					variant={incidentFilter === option.id ? "default" : "outline"}
					size="sm"
					onclick={() => (incidentFilter = option.id)}
				>
					{option.label}
					<Badge variant="secondary" class="ml-1 rounded-sm px-1.5">{option.count}</Badge>
				</Button>
			{/each}
		</div>
		{#if incidentCounts.total === 0}
			<Empty class="min-h-40 border border-dashed bg-surface-1/50">
				<EmptyHeader><EmptyTitle>No active incident signals</EmptyTitle><EmptyDescription>Scope looks clean.</EmptyDescription></EmptyHeader>
				<Button type="button" variant="outline" onclick={() => onOpenResources()}>
					<ExternalLink data-icon="inline-start" />
					Open resources
				</Button>
			</Empty>
		{:else if incidentGroups.length === 0}
			<Empty class="min-h-40 border border-dashed bg-surface-1/50">
				<EmptyHeader>
					<EmptyTitle>No matching incident signals</EmptyTitle>
					<EmptyDescription>Change severity filter to see other active signals.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			{#each incidentGroups as group}
				<Card size="sm" elevation="flat">
					<CardHeader><CardTitle>{group.label}</CardTitle><CardDescription>{group.items.length} signal rows</CardDescription></CardHeader>
					<CardContent class="overflow-x-auto p-0">
						<table class="w-full min-w-[980px] table-fixed border-collapse text-sm">
							<thead>
								<tr>
									{#each ["Resource", "Severity", "Scope", "Signals", "Latest warning", "Open"] as header}
										<th class="border-b px-3 py-2 text-left text-xs font-semibold uppercase text-muted-foreground">
											{header}
										</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each group.items as item (`${item.resource.cluster}:${item.resource.kind}:${item.resource.namespace ?? ""}:${item.resource.name}`)}
									<tr class="border-b align-top last:border-b-0">
										<td class="px-3 py-2">
											<div class="truncate font-medium">{item.resource.kind}/{item.resource.name}</div>
											<div class="mt-1 flex flex-wrap gap-1 text-xs text-muted-foreground">
												{#if item.resource.status}<Badge variant="outline">{item.resource.status}</Badge>{/if}
												{#if item.resource.ready}<Badge variant="outline">Ready {item.resource.ready}</Badge>{/if}
												{#if item.resource.restarts && item.resource.restarts > 0}<Badge variant="outline">{item.resource.restarts} restarts</Badge>{/if}
											</div>
										</td>
										<td class="px-3 py-2">{incidentSeverityLabel(item)}</td>
										<td class="truncate px-3 py-2">{incidentScopeLabel(item)}</td>
										<td class="px-3 py-2 text-xs">{incidentSignalSummary(item)}</td>
										<td class="truncate px-3 py-2 text-xs">{incidentWarningSummary(item)}</td>
										<td class="px-3 py-2 text-right">
											<Button
												type="button"
												variant="ghost"
												size="sm"
												onclick={() => onResourceSelect(item.resource, treeNodeForResource(item.resource))}
											>
												<ExternalLink data-icon="inline-start" />
												Details
											</Button>
										</td>
									</tr>
								{/each}
							</tbody>
						</table>
					</CardContent>
				</Card>
			{/each}
		{/if}
	</SurfaceFrame>

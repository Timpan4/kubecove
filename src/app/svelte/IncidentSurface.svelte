<script lang="ts">
	import { AlertTriangle, ExternalLink, RotateCcw } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import { Badge, Button, Empty, EmptyDescription, EmptyHeader, EmptyTitle, Spinner } from "@/components/ui/svelte";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import type { IncidentCockpitItem } from "@/lib/types";
	import type { IncidentFilter } from "@/features/incidents/helpers";
	import { cnfast } from "@/lib/utils";
	import {
		incidentCaseSummary,
		incidentCaseTitle,
		incidentDetailPivots,
		incidentItemKey,
		incidentKnownSummary,
		incidentMissingSummary,
		incidentNextSummary,
		incidentResourcesHealthFilter as incidentResourcesHealthFilterFor,
		incidentScopeLabel,
		incidentSeverityLabel,
		incidentSignalSummary,
		incidentWarningSummary,
		isIncidentResourceSelected,
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
		selectedResource = null,
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
	} = $props();

	const visibleIncidents = $derived(
		(incidentGroups as Array<{ items: IncidentCockpitItem[] }>).flatMap(
			(group) => group.items,
		),
	);
	const selectedIncident = $derived(
		visibleIncidents.find((item) => isIncidentResourceSelected(item, selectedResource)) ?? null,
	);

	function inspectIncident(item: IncidentCockpitItem, tab: PathStateDetailTab = "details") {
		onResourceInspect(item.resource, tab);
	}

	function resourceLabel(item: IncidentCockpitItem): string {
		return `${item.resource.kind}/${item.resource.name}`;
	}

	function severityClass(item: IncidentCockpitItem): string {
		if (item.severity === "degraded") return "border-l-destructive";
		if (item.severity === "attention") return "border-l-amber-500";
		if (item.severity === "restarted") return "border-l-sky-500";
		return "border-l-muted-foreground";
	}
</script>

<SurfaceFrame icon={AlertTriangle} title="Incident Cockpit" query={incidentsQuery} errorLabel="Incidents unavailable" wide>
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
				Open resources
			</Button>
		</div>
	</div>

	{#if incidentsQuery.data?.warnings.length}
		<FriendlyError
			mode="compact"
			error={incidentsQuery.data.warnings.join(" ")}
			context={{
				operation: "resourcesLoad",
				fallbackTitle: "Partial incident data",
				partial: true,
			}}
		/>
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
				<Badge variant="secondary" class="ml-1 rounded-sm px-1.5 tabular-nums">{option.count}</Badge>
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
		<div class="grid min-w-0 gap-4 xl:grid-cols-[minmax(18rem,0.42fr)_minmax(0,0.58fr)]">
			<section class="min-w-0 rounded-md border bg-surface-1">
				<header class="flex items-start justify-between gap-3 border-b px-3 py-3">
					<div class="min-w-0">
						<h3 class="text-sm font-semibold">Signal queue</h3>
						<p class="mt-1 text-xs text-muted-foreground">Grouped by ownership, sorted by severity and recency.</p>
					</div>
					<Badge variant="secondary" class="tabular-nums">{visibleIncidents.length}</Badge>
				</header>
				<div class="max-h-[58vh] min-h-0 overflow-y-auto p-3">
					<div class="flex flex-col gap-3">
						{#each incidentGroups as group}
							<section class="flex flex-col gap-2">
								<div class="truncate text-xs font-semibold uppercase text-muted-foreground">{group.label}</div>
								{#each group.items as item (incidentItemKey(item))}
									<button
										type="button"
										class={cnfast(
											"w-full rounded-md border border-l-4 bg-background/35 p-3 text-left transition hover:border-primary/50 hover:bg-primary/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
											severityClass(item),
											isIncidentResourceSelected(item, selectedResource) && "border-primary bg-primary/10",
										)}
										onclick={() => inspectIncident(item)}
									>
										<div class="flex min-w-0 items-start justify-between gap-3">
											<div class="min-w-0">
												<div class="truncate text-sm font-semibold">{resourceLabel(item)}</div>
												<div class="mt-1 truncate text-xs text-muted-foreground">{incidentScopeLabel(item)}</div>
											</div>
											<Badge variant={item.severity === "degraded" ? "destructive" : "outline"}>{incidentSeverityLabel(item)}</Badge>
										</div>
										<div class="mt-2 flex flex-wrap gap-1.5">
											{#if item.resource.status}<Badge variant="outline">{item.resource.status}</Badge>{/if}
											{#if item.resource.ready}<Badge variant="outline">Ready {item.resource.ready}</Badge>{/if}
											{#if item.resource.restarts && item.resource.restarts > 0}<Badge variant="outline">{item.resource.restarts} restarts</Badge>{/if}
											{#if item.warningEventCount > 0}<Badge variant="outline">{item.warningEventCount} warnings</Badge>{/if}
										</div>
										<p class="mt-2 line-clamp-2 text-xs leading-relaxed text-muted-foreground">{incidentSignalSummary(item)}</p>
									</button>
								{/each}
							</section>
						{/each}
					</div>
				</div>
			</section>

			<section class="min-w-0 rounded-md border bg-surface-1">
				{#if selectedIncident}
					<div class="border-b px-4 py-3">
						<div class="flex flex-wrap items-start justify-between gap-3">
							<div class="min-w-0">
								<h3 class="break-words text-base font-semibold">{incidentCaseTitle(selectedIncident)}</h3>
								<p class="mt-1 text-xs text-muted-foreground">{incidentScopeLabel(selectedIncident)}</p>
							</div>
							<Badge variant={selectedIncident.severity === "degraded" ? "destructive" : "outline"}>{incidentSeverityLabel(selectedIncident)}</Badge>
						</div>
						<p class="mt-3 text-sm leading-relaxed">{incidentCaseSummary(selectedIncident)}</p>
					</div>
					<div class="grid gap-3 p-4">
						<div class="grid gap-3 lg:grid-cols-3">
							<section class="rounded-md border bg-background/35 p-3">
								<div class="text-xs font-semibold uppercase text-muted-foreground">Known</div>
								<p class="mt-2 text-xs leading-relaxed">{incidentKnownSummary(selectedIncident)}</p>
							</section>
							<section class="rounded-md border bg-background/35 p-3">
								<div class="text-xs font-semibold uppercase text-muted-foreground">Missing</div>
								<p class="mt-2 text-xs leading-relaxed">{incidentMissingSummary(selectedIncident)}</p>
							</section>
							<section class="rounded-md border bg-background/35 p-3">
								<div class="text-xs font-semibold uppercase text-muted-foreground">Next</div>
								<p class="mt-2 text-xs leading-relaxed">{incidentNextSummary(selectedIncident)}</p>
							</section>
						</div>

						<div class="grid grid-cols-2 gap-2 text-xs md:grid-cols-4">
							<div class="rounded-md border bg-background/35 p-2"><div class="text-muted-foreground">Namespace</div><div class="mt-1 truncate font-semibold">{selectedIncident.resource.namespace ?? "cluster"}</div></div>
							<div class="rounded-md border bg-background/35 p-2"><div class="text-muted-foreground">Kind</div><div class="mt-1 truncate font-semibold">{selectedIncident.resource.kind}</div></div>
							<div class="rounded-md border bg-background/35 p-2"><div class="text-muted-foreground">Warnings</div><div class="mt-1 font-semibold tabular-nums">{selectedIncident.warningEventCount}</div></div>
							<div class="rounded-md border bg-background/35 p-2"><div class="text-muted-foreground">Latest</div><div class="mt-1 truncate font-semibold">{incidentWarningSummary(selectedIncident)}</div></div>
						</div>

						<div class="rounded-md border bg-background/35 p-3">
							<div class="mb-2 text-xs font-semibold uppercase text-muted-foreground">Read-only pivots</div>
							<div class="flex flex-wrap gap-2">
								{#each incidentDetailPivots(selectedIncident) as pivot}
									<Button
										type="button"
										variant={pivot.id === "details" ? "default" : "outline"}
										size="sm"
										disabled={!pivot.enabled}
										onclick={() => inspectIncident(selectedIncident, pivot.tab)}
									>
										{pivot.label}
									</Button>
								{/each}
								<Button
									type="button"
									variant="outline"
									size="sm"
									onclick={() => onResourceSelect(selectedIncident.resource, treeNodeForResource(selectedIncident.resource))}
								>
									<ExternalLink data-icon="inline-start" />
									Open in Resources
								</Button>
							</div>
						</div>
					</div>
				{:else}
					<Empty class="min-h-80 border-0 bg-transparent">
						<EmptyHeader>
							<EmptyTitle>Choose an incident signal</EmptyTitle>
							<EmptyDescription>Select a queue item to inspect details without leaving Incident Cockpit.</EmptyDescription>
						</EmptyHeader>
					</Empty>
				{/if}
			</section>
		</div>
	{/if}
</SurfaceFrame>

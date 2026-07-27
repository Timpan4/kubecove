<script lang="ts">
	import { AlertTriangle, ExternalLink, RotateCcw } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import SurfaceFrame from "@/components/SurfaceFrame.svelte";
	import {
		Badge,
		Button,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
	} from "@/components/ui/svelte";
	import type { HealthFilter } from "@/features/resources";
	import type { PathStateDetailTab } from "@/lib/path-state";
	import type {
		IncidentCockpitItem,
		IncidentCockpitSummary,
		ResourceSummary,
	} from "@/lib/types";
	import type { SavedWorkspace } from "@/lib/workspace-model";
	import type { IncidentEnrichmentState, IncidentGuidance } from "./guidance";
	import IncidentGuide from "./IncidentGuide.svelte";
	import IncidentQueue from "./IncidentQueue.svelte";
	import {
		type IncidentCounts,
		type IncidentFilter,
		type IncidentFilterOption,
		incidentResourcesHealthFilter as incidentResourcesHealthFilterFor,
	} from "./model";

	type IncidentQuery = {
		data?: IncidentCockpitSummary;
		isPending: boolean;
		isError: boolean;
		error: unknown;
		isFetching: boolean;
		refetch: () => Promise<unknown>;
	};

	let {
		workspace,
		incidentsQuery,
		incidentCounts,
		incidentFilter = $bindable("all" as IncidentFilter),
		incidentFilterOptions,
		incidentGroups,
		selectedKey,
		selectedIncident,
		guidance,
		detailsState,
		topologyState,
		emptyState,
		visibleIncidentCount,
		onIncidentSelect,
		onOpenResources,
		onResourceInspect,
		onResourceSelect,
	}: {
		workspace: SavedWorkspace;
		incidentsQuery: IncidentQuery;
		incidentCounts: IncidentCounts;
		incidentFilter?: IncidentFilter;
		incidentFilterOptions: IncidentFilterOption[];
		incidentGroups: Array<{ label: string; items: IncidentCockpitItem[] }>;
		selectedKey: string | null;
		selectedIncident: IncidentCockpitItem | null;
		guidance: IncidentGuidance | null;
		detailsState: IncidentEnrichmentState;
		topologyState: IncidentEnrichmentState;
		emptyState: "clean" | "filtered" | "ready";
		visibleIncidentCount: number;
		onIncidentSelect: (key: string) => void;
		onOpenResources: (
			namespace?: string | string[],
			initialSearch?: string,
			initialGitOpsFilter?: string,
			initialHealthFilter?: HealthFilter,
		) => void;
		onResourceInspect: (resource: ResourceSummary, detailTab?: PathStateDetailTab) => void;
		onResourceSelect: (resource: ResourceSummary) => void;
	} = $props();

	const overviewMetrics = $derived([
		{ label: "All", value: incidentCounts.total, tone: "" },
		{ label: "Degraded", value: incidentCounts.degraded, tone: "text-destructive" },
		{ label: "Attention", value: incidentCounts.attention, tone: "text-amber-600 dark:text-amber-400" },
		{ label: "Restarted", value: incidentCounts.restarted, tone: "text-sky-600 dark:text-sky-400" },
		{ label: "Warnings", value: incidentCounts.warning, tone: "text-muted-foreground" },
	]);
</script>

<SurfaceFrame
	icon={AlertTriangle}
	title="Incident Cockpit"
	query={incidentsQuery}
	errorLabel="Incidents unavailable"
	wide
>
	<div class="@container flex min-w-0 flex-col gap-3">
		<section
			class="grid min-w-0 gap-3 rounded-md border bg-surface-1 p-3 @3xl:grid-cols-[minmax(12rem,1.5fr)_repeat(5,minmax(4.5rem,0.55fr))] @5xl:grid-cols-[minmax(12rem,1.5fr)_repeat(5,minmax(4.5rem,0.55fr))_auto] @3xl:items-center"
		>
			<div class="min-w-0">
				<div class="truncate text-sm font-semibold">{workspace.name}</div>
				<div class="mt-1 truncate text-xs text-muted-foreground">
					{workspace.scope.namespaces.length === 0
						? "Scanning all namespaces"
						: `Scanning ${workspace.scope.namespaces.join(", ")}`}
				</div>
			</div>

			<div class="col-span-full grid grid-cols-2 gap-px overflow-hidden rounded-md border bg-border @md:grid-cols-5 @3xl:col-span-5">
				{#each overviewMetrics as metric}
					<div class="min-w-0 bg-background/80 px-3 py-2">
						<div class="truncate text-[0.625rem] font-semibold uppercase text-muted-foreground">
							{metric.label}
						</div>
						<div class={`mt-0.5 text-base font-semibold tabular-nums ${metric.tone}`}>{metric.value}</div>
					</div>
				{/each}
			</div>

			<div class="col-span-full flex flex-wrap gap-2 @5xl:col-span-1 @5xl:justify-self-end">
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
		</section>

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

		<div class="flex flex-wrap gap-2" aria-label="Filter incident signals">
			{#each incidentFilterOptions as option}
				<Button
					type="button"
					variant={incidentFilter === option.id ? "default" : "outline"}
					size="sm"
					aria-pressed={incidentFilter === option.id}
					onclick={() => (incidentFilter = option.id)}
				>
					{option.label}
					<Badge variant="secondary" class="ml-1 rounded-sm px-1.5 tabular-nums">
						{option.count}
					</Badge>
				</Button>
			{/each}
		</div>

		{#if emptyState === "clean"}
			<Empty class="min-h-40 border border-dashed bg-surface-1/50">
				<EmptyHeader>
					<EmptyTitle>No active incident signals</EmptyTitle>
					<EmptyDescription>Scope looks clean.</EmptyDescription>
				</EmptyHeader>
				<Button type="button" variant="outline" onclick={() => onOpenResources()}>
					<ExternalLink data-icon="inline-start" />
					Open resources
				</Button>
			</Empty>
		{:else if emptyState === "filtered"}
			<Empty class="min-h-40 border border-dashed bg-surface-1/50">
				<EmptyHeader>
					<EmptyTitle>No matching incident signals</EmptyTitle>
					<EmptyDescription>Change severity filter to see other active signals.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			<div class="grid min-w-0 gap-3 @4xl:h-[calc(100dvh-20rem)] @4xl:min-h-[32rem] @4xl:grid-cols-[minmax(18rem,0.42fr)_minmax(0,0.58fr)]">
				<IncidentQueue
					groups={incidentGroups}
					{selectedKey}
					visibleCount={visibleIncidentCount}
					onSelect={onIncidentSelect}
				/>
				<IncidentGuide
					{selectedIncident}
					{guidance}
					{detailsState}
					{topologyState}
					{onResourceInspect}
					{onResourceSelect}
				/>
			</div>
		{/if}
	</div>
</SurfaceFrame>

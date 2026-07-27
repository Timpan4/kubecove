<script lang="ts">
	import { Search } from "lucide-svelte";
	import FriendlyError from "@/components/FriendlyError.svelte";
	import {
		Badge,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Input,
		SegmentedControl,
		Spinner,
		TabsContent,
	} from "@/components/ui/svelte";
	import type { ResourceEventSummary } from "@/lib/types";
	import {
		buildEventSummary,
		EVENT_TYPE_FILTER_OPTIONS,
		filterResourceEvents,
		type EventTypeFilter,
	} from "./eventsTabModel";

	type EventsQueryState = {
		isPending: boolean;
		isError: boolean;
		error?: unknown;
	};

	let {
		eventsQuery,
		sortedEvents,
		formatEventExactTime,
		formatEventCompactTime,
	}: {
		eventsQuery: EventsQueryState;
		sortedEvents: ResourceEventSummary[];
		formatEventExactTime: (event: ResourceEventSummary) => string;
		formatEventCompactTime: (event: ResourceEventSummary) => string;
	} = $props();

	let eventSearch = $state("");
	let eventTypeFilter = $state<EventTypeFilter>("all");

	const eventSummary = $derived(buildEventSummary(sortedEvents));
	const visibleEvents = $derived(
		filterResourceEvents(sortedEvents, eventTypeFilter, eventSearch),
	);

	function eventToneClass(eventType: string): string {
		return eventType.toLocaleLowerCase() === "warning"
			? "border-amber-500/40 bg-amber-500/10 text-amber-200"
			: "border-emerald-500/30 bg-emerald-500/10 text-emerald-200";
	}

	function sourceLabel(event: ResourceEventSummary): string {
		return event.source.trim() || "unknown";
	}

	function eventAgeLabel(event: ResourceEventSummary): string {
		const value = event.lastSeen.trim();
		if (!value) return "-";
		return value.endsWith("ago") ? value : `${value} ago`;
	}

	function eventKey(event: ResourceEventSummary, index: number): string {
		return `${index}:${event.eventType}:${event.reason}:${event.lastSeenAt ?? event.lastSeen}:${event.source}:${event.message}`;
	}
</script>

<TabsContent value="events">
	{#if eventsQuery.isPending}
		<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
			<Spinner />
			<span>Loading events</span>
		</div>
	{:else if eventsQuery.isError}
		<FriendlyError
			mode="compact"
			error={eventsQuery.error}
			context={{ operation: "eventsLoad", fallbackTitle: "Failed to load events", partial: true }}
		/>
	{:else if sortedEvents.length === 0}
		<Empty class="min-h-32 border border-dashed">
			<EmptyHeader>
				<EmptyTitle>No events</EmptyTitle>
				<EmptyDescription>No Kubernetes events returned for this resource.</EmptyDescription>
			</EmptyHeader>
		</Empty>
	{:else}
		<div class="flex min-w-0 flex-col gap-3">
			<div class="grid min-w-0 grid-cols-2 gap-2 xl:grid-cols-5">
				<div class="min-w-0 rounded-md border bg-background/40 p-2">
					<div class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
						Total
					</div>
					<div class="mt-1 truncate text-sm font-semibold tabular-nums">
						{eventSummary.total} events
					</div>
				</div>
				<div class="min-w-0 rounded-md border bg-background/40 p-2">
					<div class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
						Warnings
					</div>
					<div
						class="mt-1 truncate text-sm font-semibold tabular-nums {eventSummary.warningCount > 0
							? 'text-amber-200'
							: 'text-emerald-200'}"
					>
						{eventSummary.warningCount}
					</div>
				</div>
				<div class="min-w-0 rounded-md border bg-background/40 p-2">
					<div class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
						Latest
					</div>
					<div class="mt-1 min-w-0 truncate text-sm font-semibold">
						{#if eventSummary.latestEvent}
							<span class="truncate">{eventSummary.latestEvent.reason}</span>
							<time
								class="ml-1 text-xs font-normal text-muted-foreground"
								datetime={eventSummary.latestEvent.lastSeenAt}
								title={formatEventExactTime(eventSummary.latestEvent)}
							>
								{formatEventCompactTime(eventSummary.latestEvent)}
							</time>
						{:else}
							-
						{/if}
					</div>
				</div>
				<div class="min-w-0 rounded-md border bg-background/40 p-2">
					<div class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
						Sources
					</div>
					<div class="mt-1 truncate text-sm font-semibold tabular-nums">
						{eventSummary.sourceCount}
					</div>
				</div>
				<div class="min-w-0 rounded-md border bg-background/40 p-2">
					<div class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
						Namespace
					</div>
					<div class="mt-1 truncate text-sm font-semibold">{eventSummary.namespaceLabel}</div>
				</div>
			</div>

			<div class="flex min-w-0 flex-col gap-2 xl:flex-row xl:items-center xl:justify-between">
				<div class="relative min-w-0 flex-1">
					<Search
						class="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground"
					/>
					<Input
						class="h-7 pl-7 text-xs"
						bind:value={eventSearch}
						placeholder="Filter message, reason, source..."
						aria-label="Filter resource events"
					/>
				</div>
				<SegmentedControl
					value={eventTypeFilter}
					options={EVENT_TYPE_FILTER_OPTIONS}
					onChange={(value) => (eventTypeFilter = value)}
					ariaLabel="Filter events by type"
				/>
			</div>

			<div class="flex min-w-0 flex-wrap gap-1.5">
				<Badge variant="outline" class="gap-1 border-emerald-500/30 text-emerald-200">
					<span class="size-1.5 rounded-full bg-emerald-400"></span>
					Normal {eventSummary.total - eventSummary.warningCount}
				</Badge>
				<Badge variant="outline" class="gap-1 border-amber-500/40 text-amber-200">
					<span class="size-1.5 rounded-full bg-amber-400"></span>
					Warnings {eventSummary.warningCount}
				</Badge>
				<Badge variant="outline">Sources {eventSummary.sourceCount}</Badge>
				<Badge variant="outline">{eventSummary.namespaceLabel}</Badge>
			</div>

			{#if visibleEvents.length === 0}
				<Empty class="min-h-28 border border-dashed">
					<EmptyHeader>
						<EmptyTitle>No matching events</EmptyTitle>
						<EmptyDescription>Adjust the search text or event type filter.</EmptyDescription>
					</EmptyHeader>
				</Empty>
			{:else}
				<div class="min-w-0 overflow-hidden rounded-md border bg-background/30">
					{#each visibleEvents as event, index (eventKey(event, index))}
						<div
							class="grid min-w-0 grid-cols-[4.75rem_0.75rem_minmax(0,1fr)] gap-2 border-b px-3 py-2 last:border-b-0 xl:grid-cols-[5.75rem_0.75rem_minmax(0,1fr)_7rem]"
						>
							<time
								class="whitespace-nowrap text-xs tabular-nums text-muted-foreground"
								datetime={event.lastSeenAt}
								title={formatEventExactTime(event)}
							>
								{formatEventCompactTime(event)}
							</time>
							<span
								class="mt-1.5 size-2 rounded-full {event.eventType.toLocaleLowerCase() ===
								'warning'
									? 'bg-amber-400 shadow-[0_0_0_3px_rgb(251_191_36_/_0.12)]'
									: 'bg-emerald-400 shadow-[0_0_0_3px_rgb(52_211_153_/_0.12)]'}"
								aria-hidden="true"
							></span>
							<div class="min-w-0">
								<div class="wrap-anywhere font-medium text-foreground">{event.message}</div>
								<div class="mt-1.5 flex min-w-0 flex-wrap gap-1.5">
									<Badge variant="outline" class={eventToneClass(event.eventType)}>
										{event.eventType}
									</Badge>
									<Badge variant="outline">{event.reason}</Badge>
									<Badge variant="outline" class="tabular-nums">count {event.count}</Badge>
									<Badge variant="outline">{sourceLabel(event)}</Badge>
									<Badge variant="outline">{event.namespace ?? "cluster"}</Badge>
								</div>
							</div>
							<div class="hidden min-w-0 text-right text-xs text-muted-foreground xl:block">
								<div class="truncate font-medium text-foreground">{sourceLabel(event)}</div>
								<time
									class="block truncate"
									datetime={event.lastSeenAt}
									title={formatEventExactTime(event)}
								>
									{eventAgeLabel(event)}
								</time>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</TabsContent>

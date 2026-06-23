<script lang="ts">
	import {
		Alert,
		AlertDescription,
		AlertTitle,
		Empty,
		EmptyDescription,
		EmptyHeader,
		EmptyTitle,
		Spinner,
		Table,
		TableBody,
		TableCell,
		TableHead,
		TableHeader,
		TableRow,
		TabsContent,
	} from "@/components/ui/svelte";
	import { getErrorMessage } from "./helpers";

	let { eventsQuery, sortedEvents, formatEventExactTime, formatEventLastSeen } = $props();
</script>

<TabsContent value="events">
		{#if eventsQuery.isPending}
			<div class="flex min-h-32 items-center justify-center gap-2 text-muted-foreground">
				<Spinner />
				<span>Loading events</span>
			</div>
		{:else if eventsQuery.isError}
			<Alert variant="destructive">
				<AlertTitle>Failed to load events</AlertTitle>
				<AlertDescription>{getErrorMessage(eventsQuery.error)}</AlertDescription>
			</Alert>
		{:else if sortedEvents.length === 0}
			<Empty class="min-h-32 border border-dashed">
				<EmptyHeader>
					<EmptyTitle>No events</EmptyTitle>
					<EmptyDescription>No Kubernetes events returned for this resource.</EmptyDescription>
				</EmptyHeader>
			</Empty>
		{:else}
			<Table>
				<TableHeader>
					<TableRow>
						<TableHead>Type</TableHead>
						<TableHead>Reason</TableHead>
						<TableHead>Count</TableHead>
						<TableHead>Last seen</TableHead>
						<TableHead>Source</TableHead>
						<TableHead>Namespace</TableHead>
						<TableHead>Message</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{#each sortedEvents as event (`${event.reason}:${event.lastSeen}:${event.message}`)}
						<TableRow>
							<TableCell>{event.eventType}</TableCell>
							<TableCell>{event.reason}</TableCell>
							<TableCell>{event.count}</TableCell>
							<TableCell>
								<time datetime={event.lastSeenAt} title={formatEventExactTime(event)}>
									{formatEventLastSeen(event)}
								</time>
							</TableCell>
							<TableCell>{event.source}</TableCell>
							<TableCell>{event.namespace ?? "-"}</TableCell>
							<TableCell class="max-w-80">{event.message}</TableCell>
						</TableRow>
					{/each}
				</TableBody>
			</Table>
		{/if}
	</TabsContent>


import { TimestampText } from "@/components/TimestampText";
import { StatusBadge } from "@/components/StatusBadge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent } from "@/components/ui/card";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { Clock, TriangleAlert } from "lucide-react";
import { useMemo } from "react";
import type { ResourceEventSummary } from "../../lib/types";
import { getErrorMessage } from "./helpers";
import { sortIncidentEvents } from "./incident-events";

interface EventsTabProps {
	events: ResourceEventSummary[] | undefined;
	eventsLoading: boolean;
	eventsError: boolean;
	eventsErr: unknown;
}

function EventList({ events }: { events: ResourceEventSummary[] }) {
	if (events.length === 0) {
		return (
			<Empty className="min-h-40 border">
				<EmptyHeader>
					<EmptyTitle>No events found</EmptyTitle>
					<EmptyDescription>
						This resource does not have events in the selected namespace.
					</EmptyDescription>
				</EmptyHeader>
			</Empty>
		);
	}

	return (
		<div className="flex flex-col gap-2">
			{events.map((event, index) => {
				const isWarning = event.eventType === "Warning";
				return (
					<Card
						size="sm"
						key={`${event.reason}:${event.lastSeen}:${index}`}
					>
						<CardContent>
						<div className="flex items-start justify-between gap-2">
							<div className="min-w-0">
								<div className="flex min-w-0 items-center gap-1.5">
									{isWarning ? (
										<TriangleAlert className="size-3.5 shrink-0 text-destructive" />
									) : (
										<Clock className="size-3.5 shrink-0 text-muted-foreground" />
									)}
									<span className="truncate text-[0.82rem] font-semibold text-foreground">
										{event.reason}
									</span>
								</div>
								{event.message && (
									<div className="mt-1.5 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
										{event.message}
									</div>
								)}
							</div>
							<StatusBadge tone={isWarning ? "error" : "info"}>
								{event.eventType}
							</StatusBadge>
						</div>
						<div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[0.6875rem] text-muted-foreground">
							<TimestampText
								relative={`${event.lastSeen} ago`}
								exact={event.lastSeenAt}
								className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
							/>
							{event.count > 1 && <span>{event.count} times</span>}
							<span>{event.source}</span>
							{event.namespace && <span>{event.namespace}</span>}
						</div>
						</CardContent>
					</Card>
				);
			})}
		</div>
	);
}

export function EventsTab({
	events,
	eventsLoading,
	eventsError,
	eventsErr,
}: EventsTabProps) {
	const sortedEvents = useMemo(
		() => (events ? sortIncidentEvents(events) : undefined),
		[events],
	);
	return (
		<>
			{eventsLoading && (
				<div className="p-6 text-center text-xs text-muted-foreground">
					<Spinner className="mx-auto mb-2 size-4" />
					<span>Loading events...</span>
				</div>
			)}
			{eventsError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to load events</AlertTitle>
					<AlertDescription>{getErrorMessage(eventsErr)}</AlertDescription>
				</Alert>
			)}
			{!eventsLoading && !eventsError && sortedEvents && (
				<EventList events={sortedEvents} />
			)}
		</>
	);
}

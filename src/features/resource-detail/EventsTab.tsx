import { TimestampText } from "@/components/TimestampText";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { Clock, TriangleAlert } from "lucide-react";
import type { ResourceEventSummary } from "../../lib/types";
import {
	ERROR_STATE_CLASS,
	LOADING_SPINNER_CLASS,
	LOADING_STATE_CLASS,
} from "./constants";
import { getErrorMessage } from "./helpers";

interface EventsTabProps {
	events: ResourceEventSummary[] | undefined;
	eventsLoading: boolean;
	eventsError: boolean;
	eventsErr: unknown;
}

function EventList({ events }: { events: ResourceEventSummary[] }) {
	if (events.length === 0) {
		return (
			<div className="rounded-md border bg-card p-4 text-xs text-muted-foreground">
				No events found for this resource.
			</div>
		);
	}

	return (
		<div className="flex flex-col gap-2">
			{events.map((event, index) => {
				const isWarning = event.eventType === "Warning";
				return (
					<div
						className="rounded-md border bg-card p-3"
						key={`${event.reason}:${event.lastSeen}:${index}`}
					>
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
							<Badge
								variant={isWarning ? "destructive" : "outline"}
								className={cn(
									"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
									!isWarning &&
										"border-sky-500/30 bg-sky-500/10 text-sky-300 dark:bg-sky-500/15",
								)}
							>
								{event.eventType}
							</Badge>
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
					</div>
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
	return (
		<>
			{eventsLoading && (
				<div className={LOADING_STATE_CLASS}>
					<div className={LOADING_SPINNER_CLASS}></div>
					<span>Loading events...</span>
				</div>
			)}
			{eventsError && (
				<div className={ERROR_STATE_CLASS}>
					<p>Error loading events: {getErrorMessage(eventsErr)}</p>
				</div>
			)}
			{!eventsLoading && !eventsError && events && (
				<EventList events={events} />
			)}
		</>
	);
}

import type { ResourceEventSummary } from "../../lib/types";

function eventTimeValue(event: ResourceEventSummary): number {
	const parsed = event.lastSeenAt ? Date.parse(event.lastSeenAt) : Number.NaN;
	return Number.isNaN(parsed) ? 0 : parsed;
}

export function sortIncidentEvents(
	events: ResourceEventSummary[],
): ResourceEventSummary[] {
	// Parse each timestamp once instead of twice per comparison.
	const keyed = events.map((event) => ({ event, time: eventTimeValue(event) }));
	keyed.sort((a, b) => {
		const warningDelta =
			Number(b.event.eventType === "Warning") -
			Number(a.event.eventType === "Warning");
		if (warningDelta !== 0) return warningDelta;
		const timeDelta = b.time - a.time;
		if (timeDelta !== 0) return timeDelta;
		const countDelta = b.event.count - a.event.count;
		if (countDelta !== 0) return countDelta;
		return a.event.reason.localeCompare(b.event.reason);
	});
	const sorted: ResourceEventSummary[] = [];
	for (const { event } of keyed) sorted.push(event);
	return sorted;
}

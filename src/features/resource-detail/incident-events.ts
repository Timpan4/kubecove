import type { ResourceEventSummary } from "../../lib/types";

function eventTimeValue(event: ResourceEventSummary): number {
	const parsed = event.lastSeenAt ? Date.parse(event.lastSeenAt) : Number.NaN;
	return Number.isNaN(parsed) ? 0 : parsed;
}

export function sortIncidentEvents(
	events: ResourceEventSummary[],
): ResourceEventSummary[] {
	return [...events].sort((a, b) => {
		const warningDelta =
			Number(b.eventType === "Warning") - Number(a.eventType === "Warning");
		if (warningDelta !== 0) return warningDelta;
		const timeDelta = eventTimeValue(b) - eventTimeValue(a);
		if (timeDelta !== 0) return timeDelta;
		const countDelta = b.count - a.count;
		if (countDelta !== 0) return countDelta;
		return a.reason.localeCompare(b.reason);
	});
}

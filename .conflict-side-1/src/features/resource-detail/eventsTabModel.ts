import type { ResourceEventSummary } from "../../lib/types";

export type EventTypeFilter = "all" | "warning" | "normal";

export const EVENT_TYPE_FILTER_OPTIONS: { value: EventTypeFilter; label: string }[] = [
	{ value: "all", label: "All" },
	{ value: "warning", label: "Warnings" },
	{ value: "normal", label: "Normal" },
];

export interface EventSummary {
	total: number;
	warningCount: number;
	sourceCount: number;
	namespaceLabel: string;
	latestEvent?: ResourceEventSummary;
}

function normalized(value: string | null | undefined): string {
	return (value ?? "").trim().toLocaleLowerCase();
}

function eventTimeValue(event: ResourceEventSummary): number {
	const parsed = event.lastSeenAt ? Date.parse(event.lastSeenAt) : Number.NaN;
	return Number.isNaN(parsed) ? Number.NEGATIVE_INFINITY : parsed;
}

function uniqueCount(values: (string | null | undefined)[]): number {
	return new Set(values.map((value) => value?.trim()).filter(Boolean)).size;
}

function namespaceLabel(events: ResourceEventSummary[]): string {
	const namespaces = new Set(
		events.map((event) => event.namespace?.trim()).filter((value): value is string => Boolean(value)),
	);
	if (namespaces.size === 0) return "cluster";
	if (namespaces.size === 1) return [...namespaces][0] ?? "cluster";
	return `${namespaces.size} namespaces`;
}

export function buildEventSummary(events: ResourceEventSummary[]): EventSummary {
	let latestEvent = events[0];
	let latestTime = latestEvent ? eventTimeValue(latestEvent) : Number.NEGATIVE_INFINITY;

	for (const event of events.slice(1)) {
		const eventTime = eventTimeValue(event);
		if (eventTime > latestTime) {
			latestEvent = event;
			latestTime = eventTime;
		}
	}

	return {
		total: events.length,
		warningCount: events.filter((event) => normalized(event.eventType) === "warning").length,
		sourceCount: uniqueCount(events.map((event) => event.source)),
		namespaceLabel: namespaceLabel(events),
		latestEvent,
	};
}

function matchesType(event: ResourceEventSummary, filter: EventTypeFilter): boolean {
	if (filter === "all") return true;
	return normalized(event.eventType) === filter;
}

function matchesSearch(event: ResourceEventSummary, query: string): boolean {
	if (!query) return true;
	return [
		event.message,
		event.reason,
		event.source,
		event.eventType,
		event.namespace ?? "",
	]
		.some((value) => normalized(value).includes(query));
}

export function filterResourceEvents(
	events: ResourceEventSummary[],
	typeFilter: EventTypeFilter,
	query: string,
): ResourceEventSummary[] {
	const searchQuery = normalized(query);
	return events.filter(
		(event) => matchesType(event, typeFilter) && matchesSearch(event, searchQuery),
	);
}

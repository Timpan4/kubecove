import type { ResourceEventSummary } from "../../lib/types";
import {
	buildEventSummary,
	filterResourceEvents,
	type EventTypeFilter,
} from "./eventsTabModel";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

const events: ResourceEventSummary[] = [
	{
		eventType: "Warning",
		reason: "BackOff",
		message: "Back-off restarting failed container api",
		count: 3,
		lastSeen: "2m ago",
		lastSeenAt: "2026-06-29T20:33:00Z",
		source: "kubelet",
		namespace: "todo",
	},
	{
		eventType: "Normal",
		reason: "Pulled",
		message: 'Container image "nginx:1.29.3-alpine" already present',
		count: 1,
		lastSeen: "1m ago",
		lastSeenAt: "2026-06-29T20:34:00Z",
		source: "kubelet",
		namespace: "todo",
	},
	{
		eventType: "Normal",
		reason: "Scheduled",
		message: "Successfully assigned todo/todo-web to solid-worker-2",
		count: 1,
		lastSeen: "3m ago",
		lastSeenAt: "2026-06-29T20:32:00Z",
		source: "default-scheduler",
		namespace: "todo",
	},
];

describe("events tab model", () => {
	test("summarizes warnings, latest event, sources, and namespace", () => {
		const summary = buildEventSummary(events);

		expect(summary.total).toBe(3);
		expect(summary.warningCount).toBe(1);
		expect(summary.latestEvent?.reason).toBe("Pulled");
		expect(summary.sourceCount).toBe(2);
		expect(summary.namespaceLabel).toBe("todo");
	});

	test("search matches message, reason, source, type, and namespace", () => {
		const cases: [string, string[]][] = [
			["nginx", ["Pulled"]],
			["scheduled", ["Scheduled"]],
			["default-scheduler", ["Scheduled"]],
			["warning", ["BackOff"]],
			["todo", ["BackOff", "Pulled", "Scheduled"]],
		];

		for (const [query, expectedReasons] of cases) {
			expect(filterResourceEvents(events, "all", query).map((event) => event.reason)).toEqual(
				expectedReasons,
			);
		}
	});

	test("type filter preserves incoming sort order", () => {
		const filters: [EventTypeFilter, string[]][] = [
			["warning", ["BackOff"]],
			["normal", ["Pulled", "Scheduled"]],
		];

		for (const [filter, expectedReasons] of filters) {
			expect(filterResourceEvents(events, filter, "").map((event) => event.reason)).toEqual(
				expectedReasons,
			);
		}
	});
});

import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	formatExactTimeOnly,
	formatExactTimestamp,
} from "@/components/timestamp-format";
import {
	buildEventSummary,
	filterResourceEvents,
} from "@/features/resource-detail/eventsTabModel";
import {
	buildIncidentSignals,
	conditionStatusTone,
	getConditionRows,
	getContainerStatusRows,
	resourceReadyLabel,
} from "@/features/resource-detail/helpers";
import { sortIncidentEvents } from "@/features/resource-detail/incident-events";
import { buildIncidentTimeline } from "@/features/resource-detail/incident-timeline";
import {
	groupIncidentTimelineByDay,
	incidentTimelineTime,
	incidentTimelineTitle,
} from "@/features/resource-detail/incident-timeline-labels";
import {
	appendParsedLogBatch,
	filterOrderedParsedLogLines,
	type LogLineInput,
	MAX_RETAINED_LOG_LINES,
	orderParsedLogLines,
	type ParsedLogLine,
} from "@/features/resource-detail/log-helpers";
import type {
	JsonObject,
	ResourceEventSummary,
	ResourceSummary,
} from "@/lib/types";

const startedAt = Date.UTC(2026, 0, 1);
const timestamp = (index: number) =>
	new Date(startedAt + index * 1_000).toISOString();

const logLines: LogLineInput[] = Array.from({ length: 1_000 }, (_, index) => {
	const level = index % 10 === 0 ? "error" : "info";
	const message = `${level} request=${index} namespace=namespace-${index % 100}`;
	const line =
		index % 3 === 0
			? `${timestamp(index)} ${message}`
			: index % 3 === 1
				? `level=${level} time=${timestamp(index)} ${message}`
				: message;
	return {
		line,
		source: {
			podName: `api-${index % 50}`,
			container: index % 4 === 0 ? "sidecar" : "api",
		},
	};
});

const events: ResourceEventSummary[] = Array.from(
	{ length: 1_000 },
	(_, index) => ({
		eventType: index % 2 === 0 ? "Warning" : "Normal",
		reason: index % 2 === 0 ? "BackOff" : "Pulled",
		message: `event ${index} for api-${index % 50}`,
		count: (index % 8) + 1,
		lastSeen: `${index + 1}s`,
		lastSeenAt: index % 53 === 0 ? undefined : timestamp(index),
		source: index % 3 === 0 ? "kubelet" : "scheduler",
		namespace: `namespace-${index % 100}`,
	}),
);

const rawStatus: JsonObject = {
	conditions: Array.from({ length: 12 }, (_, index) => ({
		type: `Condition${index}`,
		status: index % 3 === 0 ? "False" : "Unknown",
		reason: index % 3 === 0 ? "Unavailable" : "Reconciling",
		message: `condition ${index}`,
		lastTransitionTime: timestamp(index),
	})),
	initContainerStatuses: Array.from({ length: 5 }, (_, index) => ({
		name: `init-${index}`,
		ready: index % 2 === 0,
		restartCount: index % 4,
		state: { terminated: { exitCode: 1, finishedAt: timestamp(index) } },
	})),
	containerStatuses: Array.from({ length: 20 }, (_, index) => ({
		name: `container-${index}`,
		ready: index % 3 !== 0,
		restartCount: index % 4,
		state:
			index % 3 === 0
				? { waiting: { reason: "CrashLoopBackOff", message: "backoff" } }
				: { running: { startedAt: timestamp(index + 100) } },
		lastState: {
			terminated: {
				reason: "Error",
				exitCode: 1,
				finishedAt: timestamp(index + 90),
			},
		},
	})),
	ephemeralContainerStatuses: Array.from({ length: 4 }, (_, index) => ({
		name: `debug-${index}`,
		ready: false,
		restartCount: 1,
		state: { waiting: { reason: "ContainerCreating" } },
	})),
};
const malformedStatus: JsonObject = {
	conditions: [
		null,
		{ type: null, status: null, reason: 42, lastTransitionTime: 1 },
		{ type: "Ready", status: "False", lastTransitionTime: "invalid-time" },
	],
	containerStatuses: [
		null,
		{
			name: null,
			ready: "false",
			restartCount: "3",
			state: { waiting: "malformed state" },
		},
	],
};
const terminalStatus: JsonObject = {
	conditions: [
		{ type: "Ready", status: "False", reason: "PodCompleted" },
		{ type: "ContainersReady", status: "False", reason: "PodCompleted" },
		{ type: "PodReadyToStartContainers", status: "False" },
	],
	containerStatuses: [
		{
			name: "completed",
			ready: false,
			restartCount: 2,
			state: {
				terminated: {
					reason: "Completed",
					exitCode: 0,
					finishedAt: "2025-12-31T23:59:00Z",
				},
			},
			lastState: {
				terminated: {
					reason: "Completed",
					exitCode: 0,
					finishedAt: "2025-12-31T23:58:00Z",
				},
			},
		},
	],
};
const staleRestartStatus: JsonObject = {
	initContainerStatuses: [
		{
			name: "old-init",
			ready: false,
			restartCount: 2,
			state: {
				terminated: {
					reason: "Completed",
					exitCode: 0,
					finishedAt: "2025-12-31T20:00:00Z",
				},
			},
			lastState: {
				terminated: {
					reason: "Completed",
					exitCode: 0,
					finishedAt: "2025-12-31T19:00:00Z",
				},
			},
		},
	],
};
const resource: ResourceSummary = {
	cluster: "prod",
	apiVersion: "v1",
	kind: "Pod",
	name: "checkout-api",
	namespace: "checkout",
	age: "1m",
	health: "degraded",
	status: "CrashLoopBackOff",
	ready: "False",
	restarts: 80,
};

function buildRetainedLogs(): ParsedLogLine[] {
	return appendParsedLogBatch([], logLines, 0);
}

function appendOverflowBatch(retained: ParsedLogLine[]): ParsedLogLine[] {
	return appendParsedLogBatch(retained, logLines, logLines.length);
}

const parsedLogLines = buildRetainedLogs();
let overflowingLogLines = buildRetainedLogs();
const latestFirstLines = orderParsedLogLines(parsedLogLines, true);
const sortedEvents = sortIncidentEvents(events);
const filteredEvents = filterResourceEvents(sortedEvents, "warning", "backoff");
const conditions = getConditionRows(rawStatus);
const containers = getContainerStatusRows(rawStatus, resource.kind);
const malformedConditions = getConditionRows(malformedStatus);
const malformedContainers = getContainerStatusRows(malformedStatus, resource.kind);
const terminalConditions = getConditionRows(terminalStatus);
const terminalReadyCondition = terminalConditions[0];
if (!terminalReadyCondition) throw new Error("Missing terminal condition fixture");
const terminalContainers = getContainerStatusRows(terminalStatus, resource.kind);
const staleRestartContainers = getContainerStatusRows(staleRestartStatus, resource.kind);
const incidentSignals = buildIncidentSignals(
	resource,
	conditions,
	sortedEvents,
	containers,
	{
		now: new Date("2026-01-01T00:30:00.000Z"),
	},
);
const timeline = buildIncidentTimeline({
	resource,
	conditions,
	events: sortedEvents,
	containers,
	logLines: parsedLogLines,
});
const timelineGroups = groupIncidentTimelineByDay(timeline, "utc");
const terminalResource: ResourceSummary = {
	...resource,
	status: "Succeeded",
	ready: "False",
	restarts: 2,
};
const terminalSignals = buildIncidentSignals(
	terminalResource,
	terminalConditions,
	[],
	terminalContainers,
	{ now: new Date("2026-01-01T00:30:00.000Z") },
);
const staleRestartSignals = buildIncidentSignals(
	{ ...resource, status: "Running", ready: "True", restarts: 2 },
	[],
	[],
	staleRestartContainers,
	{ now: new Date("2026-01-01T00:30:00.000Z") },
);
const boundaryTimelineEvents: ResourceEventSummary[] = [
	{
		eventType: "Warning",
		reason: "BackOff",
		message: "prior day warning",
		count: 1,
		lastSeen: "1s",
		lastSeenAt: "2025-12-31T23:59:59.000Z",
		source: "kubelet",
		namespace: "checkout",
	},
	{
		eventType: "Warning",
		reason: "FailedScheduling",
		message: "same time scheduling warning",
		count: 1,
		lastSeen: "1s",
		lastSeenAt: "2026-01-01T00:00:01.000Z",
		source: "scheduler",
		namespace: "checkout",
	},
	{
		eventType: "Warning",
		reason: "BackOff",
		message: "same time restart warning",
		count: 1,
		lastSeen: "1s",
		lastSeenAt: "2026-01-01T00:00:01.000Z",
		source: "kubelet",
		namespace: "checkout",
	},
	{
		eventType: "Warning",
		reason: "BackOff",
		message: "same time restart warning",
		count: 1,
		lastSeen: "1s",
		lastSeenAt: "2026-01-01T00:00:01.000Z",
		source: "kubelet",
		namespace: "checkout",
	},
	{
		eventType: "Warning",
		reason: "InvalidTime",
		message: "warning without a usable time",
		count: 1,
		lastSeen: "unknown",
		lastSeenAt: "not-a-time",
		source: "scheduler",
		namespace: "checkout",
	},
];
const sortedBoundaryEvents = sortIncidentEvents(boundaryTimelineEvents);
const boundaryTimeline = buildIncidentTimeline({
	resource,
	conditions: [],
	events: sortedBoundaryEvents,
});
const boundaryDayGroups = groupIncidentTimelineByDay(boundaryTimeline, "utc");
const invalidTimeTimelineItem = boundaryTimeline.find(
	(item) => item.title === "Warning InvalidTime",
);

assert.equal(logLines.length, 1_000);
assert.equal(parsedLogLines.length, MAX_RETAINED_LOG_LINES);
overflowingLogLines = appendOverflowBatch(overflowingLogLines);
assert.equal(overflowingLogLines.length, MAX_RETAINED_LOG_LINES);
assert.equal(overflowingLogLines[0].index, logLines.length);
assert.equal(events.length, 1_000);
assert.equal(buildEventSummary(sortedEvents).warningCount, 500);
assert.equal(filteredEvents.length, 500);
assert(sortedEvents.indexOf(events[0]) < sortedEvents.indexOf(events[424]));
assert.equal(events[0].eventType, events[424].eventType);
assert.equal(events[0].lastSeenAt, events[424].lastSeenAt);
assert.equal(events[0].count, events[424].count);
assert.equal(events[0].reason, events[424].reason);
assert.equal(conditions.length, 12);
assert.equal(containers.length, 29);
assert.equal(malformedConditions.length, 2);
assert.equal(malformedConditions[0]?.type, "Condition");
assert.equal(malformedConditions[0]?.status, "Unknown");
assert.equal(malformedConditions[1]?.lastTransitionTime, "invalid-time");
assert.equal(malformedContainers.length, 1);
assert.equal(malformedContainers[0]?.name, "container");
assert.equal(malformedContainers[0]?.ready, undefined);
assert.equal(malformedContainers[0]?.restartCount, 0);
assert.equal(malformedContainers[0]?.state, undefined);
assert.equal(resourceReadyLabel(terminalResource), "Completed");
assert.equal(
	conditionStatusTone(terminalReadyCondition, terminalResource),
	"neutral",
);
assert.equal(terminalSignals.length, 0);
assert.equal(staleRestartSignals.some((signal) => signal.id === "restarts"), false);
assert.equal(
	boundaryTimeline.filter((item) => item.source === "event").length,
	4,
);
assert.deepEqual(
	boundaryDayGroups.map((group) => group.key),
	["2025-12-31", "2026-01-01", "unknown"],
);
assert.equal(boundaryDayGroups[1]?.items.length, 2);
assert.equal(boundaryDayGroups[2]?.items.length, 1);
assert.equal(
	invalidTimeTimelineItem && incidentTimelineTime(invalidTimeTimelineItem, "utc"),
	"No time",
);
assert(
	incidentSignals.length > 0 &&
		timeline.length > 0 &&
		timelineGroups.length > 0,
);
assert.equal(
	filterOrderedParsedLogLines(latestFirstLines, ""),
	latestFirstLines,
);

describe("F13 F14 F15 resource detail workloads with folded F11 and F24", () => {
	bench("F13 append and retain a 1k-line stream buffer", () => {
		buildRetainedLogs();
	});

	bench("F13 steady-state eviction at the 1k-line retention bound", () => {
		overflowingLogLines = appendOverflowBatch(overflowingLogLines);
	});

	bench("F13 batched steady-state eviction at the 1k-line retention bound", () => {
		let retained = parsedLogLines;
		for (let offset = 0; offset < logLines.length; offset += 64) {
			retained = appendParsedLogBatch(
				retained,
				logLines.slice(offset, offset + 64),
				logLines.length + offset,
			);
		}
		void retained;
	});

	bench(
		"F13 mounted retained log timestamp presentation with folded F24",
		() => {
			for (const line of parsedLogLines) {
				formatExactTimestamp(line.timestamp, "utc", "millisecond");
				formatExactTimeOnly(line.timestamp, "utc", "millisecond");
			}
		},
	);

	bench("F13 retained query scan on preordered parsed lines", () => {
		filterOrderedParsedLogLines(latestFirstLines, "error");
	});

	bench("F13 latest-first ordering invalidation on the retained buffer", () => {
		orderParsedLogLines(parsedLogLines, true);
	});

	bench("F14 event arrival sort and summary", () => {
		const arrived = sortIncidentEvents(events);
		buildEventSummary(arrived);
	});

	bench("F14 retained event search and mounted timestamp presentation", () => {
		const visible = filterResourceEvents(sortedEvents, "warning", "backoff");
		for (const event of visible) {
			void (
				formatExactTimestamp(event.lastSeenAt, "utc", "millisecond") ??
				event.lastSeen
			);
			void (
				formatExactTimeOnly(event.lastSeenAt, "utc", "second") ?? event.lastSeen
			);
		}
	});

	bench(
		"F15 cold signals, timeline assembly, grouping, titles, and times",
		() => {
			const nextConditions = getConditionRows(rawStatus);
			const nextContainers = getContainerStatusRows(rawStatus, resource.kind);
			const nextEvents = sortIncidentEvents(events);
			buildIncidentSignals(
				resource,
				nextConditions,
				nextEvents,
				nextContainers,
				{
					now: new Date("2026-01-01T00:30:00.000Z"),
				},
			);
			const nextTimeline = buildIncidentTimeline({
				resource,
				conditions: nextConditions,
				events: nextEvents,
				containers: nextContainers,
				logLines: parsedLogLines,
			});
			const groups = groupIncidentTimelineByDay(nextTimeline, "utc");
			for (const item of nextTimeline) {
				incidentTimelineTitle(item, resource.kind);
				incidentTimelineTime(item, "utc");
			}
		const malformedRows = getConditionRows(malformedStatus);
		const malformedContainerRows = getContainerStatusRows(
			malformedStatus,
			resource.kind,
		);
		buildIncidentSignals(
			resource,
			malformedRows,
			nextEvents,
			malformedContainerRows,
			{ now: new Date("2026-01-01T00:30:00.000Z") },
		);
		buildIncidentSignals(
			terminalResource,
			terminalConditions,
			nextEvents,
			terminalContainers,
			{ now: new Date("2026-01-01T00:30:00.000Z") },
		);
		buildIncidentSignals(
			resource,
			[],
			[],
			staleRestartContainers,
			{ now: new Date("2026-01-01T00:30:00.000Z") },
		);
		const boundaryRows = sortIncidentEvents(boundaryTimelineEvents);
		const edgeTimeline = buildIncidentTimeline({
			resource,
			conditions: [],
			events: boundaryRows,
		});
		groupIncidentTimelineByDay(edgeTimeline, "utc");
		for (const item of edgeTimeline) {
			incidentTimelineTitle(item, resource.kind);
			incidentTimelineTime(item, "utc");
		}
			void groups;
		},
	);

	bench("F15 retained timeline grouping, titles, and timestamps", () => {
		groupIncidentTimelineByDay(timeline, "utc");
		for (const item of timeline) {
			incidentTimelineTitle(item, resource.kind);
			incidentTimelineTime(item, "utc");
		}
	});
});

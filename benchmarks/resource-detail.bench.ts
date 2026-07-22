import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	buildEventSummary,
	filterResourceEvents,
} from "@/features/resource-detail/eventsTabModel";
import type {
	ConditionRow,
	ContainerStatusRow,
} from "@/features/resource-detail/helpers";
import { sortIncidentEvents } from "@/features/resource-detail/incident-events";
import { buildIncidentTimeline } from "@/features/resource-detail/incident-timeline";
import {
	logLineSearchText,
	orderedLogLines,
} from "@/features/resource-detail/log-helpers";
import type { LogLineInput } from "@/features/resource-detail/log-helpers";
import type { ResourceEventSummary, ResourceSummary } from "@/lib/types";

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

const conditions: ConditionRow[] = Array.from({ length: 12 }, (_, index) => ({
	type: `Condition${index}`,
	status: index % 3 === 0 ? "False" : "Unknown",
	reason: index % 3 === 0 ? "Unavailable" : "Reconciling",
	message: `condition ${index}`,
	lastTransitionTime: timestamp(index),
}));
const containers: ContainerStatusRow[] = Array.from(
	{ length: 20 },
	(_, index) => ({
		name: `container-${index}`,
		ready: index % 3 !== 0,
		restartCount: index % 4,
		state: index % 3 === 0 ? "waiting" : "running",
		reason: index % 3 === 0 ? "CrashLoopBackOff" : undefined,
		lastReason: index % 4 ? "Error" : undefined,
		lastExitCode: index % 4 ? 1 : 0,
		startedAt: timestamp(index + 100),
		lastFinishedAt: timestamp(index + 90),
	}),
);
const resource: ResourceSummary = {
	cluster: "prod",
	apiVersion: "v1",
	kind: "Pod",
	name: "checkout-api",
	namespace: "checkout",
	age: "1m",
	status: "CrashLoopBackOff",
};
const parsedLogLines = orderedLogLines(logLines, false);
const sortedEvents = sortIncidentEvents(events);

assert.equal(logLines.length, 1_000);
assert.equal(events.length, 1_000);
assert.equal(events.filter((event) => event.eventType === "Warning").length, 500);
assert(events.some((event) => event.lastSeenAt === undefined));

describe("resource detail", () => {
	bench("parse + order + search 1k log lines", () => {
		orderedLogLines(logLines, true).filter((line) =>
			logLineSearchText(line).includes("error"),
		);
	});

	bench("sort + summarize + filter 1k events", () => {
		const sorted = sortIncidentEvents(events);
		buildEventSummary(sorted);
		filterResourceEvents(sorted, "warning", "backoff");
	});

	bench("build incident timeline from detail signals", () => {
		buildIncidentTimeline({
			resource,
			conditions,
			events: sortedEvents,
			containers,
			logLines: parsedLogLines,
		});
	});
});

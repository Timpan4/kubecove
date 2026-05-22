import { describe, expect, test } from "bun:test";
import type { ResourceEventSummary, ResourceSummary } from "../src/lib/types";
import {
	buildIncidentSignals,
	type ConditionRow,
	type ContainerStatusRow,
} from "../src/features/resource-detail/helpers";
import { buildIncidentTimeline } from "../src/features/resource-detail/incident-timeline";
import { sortIncidentEvents } from "../src/features/resource-detail/incident-events";
import { parseLogLine } from "../src/features/resource-detail/log-helpers";
import { filterResourcesByHealth } from "../src/features/resources/helpers";

function resource(overrides: Partial<ResourceSummary> = {}): ResourceSummary {
	return {
		cluster: "kind-dev",
		kind: "Pod",
		name: "api-0",
		namespace: "default",
		age: "3m",
		status: "Running",
		ready: "true",
		...overrides,
	};
}

function event(overrides: Partial<ResourceEventSummary>): ResourceEventSummary {
	return {
		eventType: "Normal",
		reason: "Pulled",
		message: "Container image pulled",
		count: 1,
		lastSeen: "1m",
		lastSeenAt: "2026-05-19T10:00:00.000Z",
		source: "kubelet",
		namespace: "default",
		...overrides,
	};
}

describe("incident workflow helpers", () => {
	test("unhealthy filtering includes degraded and attention resources", () => {
		const rows = [
			resource({ name: "healthy", status: "Running", ready: "true" }),
			resource({ name: "pending", status: "Pending" }),
			resource({ name: "failed", status: "Failed" }),
			resource({ name: "restarted", restarts: 2 }),
		];

		expect(filterResourcesByHealth(rows, "unhealthy").map((row) => row.name)).toEqual([
			"pending",
			"failed",
			"restarted",
		]);
		expect(filterResourcesByHealth(rows, "healthy").map((row) => row.name)).toEqual([
			"healthy",
		]);
		expect(filterResourcesByHealth(rows, "degraded").map((row) => row.name)).toEqual([
			"failed",
		]);
		expect(filterResourcesByHealth(rows, "restarted").map((row) => row.name)).toEqual([
			"restarted",
		]);
	});

	test("incident signals combine bad status, conditions, warning events, and actionable restarts", () => {
		const conditions: ConditionRow[] = [
			{
				type: "Ready",
				status: "False",
				reason: "ContainersNotReady",
				lastTransitionTime: "2026-05-19T09:55:00.000Z",
			},
		];
		const containers: ContainerStatusRow[] = [
			{
				name: "api",
				type: "container",
				ready: false,
				restartCount: 3,
				state: "waiting",
				reason: "CrashLoopBackOff",
				lastState: "terminated",
				lastReason: "Error",
				lastExitCode: 1,
				lastFinishedAt: "2026-05-19T09:59:00.000Z",
			},
		];
		const signals = buildIncidentSignals(
			resource({ status: "CrashLoopBackOff", ready: "false", restarts: 3 }),
			conditions,
			[event({ eventType: "Warning", reason: "BackOff", count: 4 })],
			containers,
			{ now: new Date("2026-05-19T10:00:00.000Z") },
		);

		expect(signals.map((signal) => signal.id)).toEqual([
			"status",
			"ready",
			"restarts",
			"condition:Ready",
			"events:warnings",
		]);
		expect(signals.find((signal) => signal.id === "restarts")?.tone).toBe(
			"warning",
		);
	});

	test("warning events sort ahead of normal events, then newest first", () => {
		const events = [
			event({
				eventType: "Normal",
				reason: "Started",
				lastSeenAt: "2026-05-19T10:05:00.000Z",
			}),
			event({
				eventType: "Warning",
				reason: "FailedMount",
				lastSeenAt: "2026-05-19T10:01:00.000Z",
			}),
			event({
				eventType: "Warning",
				reason: "BackOff",
				lastSeenAt: "2026-05-19T10:03:00.000Z",
			}),
		];

		expect(sortIncidentEvents(events).map((item) => item.reason)).toEqual([
			"BackOff",
			"FailedMount",
			"Started",
		]);
		expect(events.map((item) => item.reason)).toEqual([
			"Started",
			"FailedMount",
			"BackOff",
		]);
	});

	test("incident timeline orders events, conditions, restarts, and log metadata", () => {
		const timeline = buildIncidentTimeline({
			resource: resource({ status: "Pending", ready: "false" }),
			conditions: [
				{
					type: "Ready",
					status: "False",
					reason: "ContainersNotReady",
					lastTransitionTime: "2026-05-19T09:55:00.000Z",
				},
			],
			events: [
				event({
					eventType: "Warning",
					reason: "BackOff",
					message: "Back-off restarting failed container",
					count: 3,
					lastSeenAt: "2026-05-19T09:59:00.000Z",
				}),
				event({
					eventType: "Normal",
					reason: "Pulled",
					lastSeenAt: "2026-05-19T09:58:00.000Z",
				}),
			],
			containers: [
				{
					name: "api",
					type: "container",
					ready: false,
					restartCount: 2,
					state: "waiting",
					reason: "CrashLoopBackOff",
					lastReason: "Error",
					lastExitCode: 1,
					lastFinishedAt: "2026-05-19T09:57:00.000Z",
				},
			],
			logLines: [
				parseLogLine("2026-05-19T10:00:00Z request failed", 0),
				parseLogLine("untimestamped", 1),
			],
		});

		expect(timeline.map((item) => item.source)).toEqual([
			"condition",
			"restart",
			"event",
			"log",
			"container",
			"status",
			"status",
		]);
		expect(timeline.find((item) => item.source === "event")?.detail).toContain(
			"3 repeats",
		);
		expect(timeline.find((item) => item.source === "log")?.detail).toBe(
			"request failed",
		);
	});

	test("incident timeline deduplicates repeated condition entries", () => {
		const timeline = buildIncidentTimeline({
			resource: resource(),
			conditions: [
				{ type: "Ready", status: "False", reason: "A" },
				{ type: "Ready", status: "False", reason: "A" },
			],
			events: [],
		});

		expect(timeline).toHaveLength(1);
		expect(timeline[0].id).toBe("condition:Ready:False");
	});

	test("incident timeline stays empty for healthy resources", () => {
		expect(
			buildIncidentTimeline({
				resource: resource({ status: "Running", ready: "true", restarts: 0 }),
				conditions: [{ type: "Ready", status: "True" }],
				events: [],
				containers: [
					{
						name: "api",
						ready: true,
						restartCount: 0,
						state: "running",
					},
				],
			}),
		).toEqual([]);
	});
});

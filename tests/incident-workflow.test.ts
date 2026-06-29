import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
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
		health: "healthy",
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

function svelteDetailSource(): string {
	return [
		"src/features/resource-detail/ResourceDetailPanel.svelte",
		"src/features/resource-detail/DetailsTab.svelte",
		"src/features/resource-detail/EventsTab.svelte",
		"src/features/resource-detail/YamlTab.svelte",
		"src/features/resource-detail/LogsTab.svelte",
	]
		.map((path) => readFileSync(path, "utf8"))
		.join("\n");
}

describe("incident workflow helpers", () => {
	test("unhealthy filtering includes degraded and attention resources", () => {
		const rows = [
			resource({ name: "healthy", status: "Running", ready: "true", health: "healthy" }),
			resource({ name: "pending", status: "Pending", health: "attention" }),
			resource({ name: "failed", status: "Failed", health: "degraded" }),
			resource({ name: "restarted", restarts: 2, health: "restarted" }),
		];

		expect(filterResourcesByHealth(rows, "unhealthy").map((row) => row.name)).toEqual([
			"pending",
			"failed",
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
			"container:api:waiting",
			"events:warnings",
			"restarts",
			"condition:Ready",
			"status",
			"ready",
		]);
		expect(signals[0]).toMatchObject({
			label: "api waiting",
			source: "container",
			tone: "error",
		});
		expect(signals.find((signal) => signal.id === "restarts")?.tone).toBe(
			"warning",
		);
	});

	test("incident signals prioritize failed containers before derived pod state", () => {
		const conditions: ConditionRow[] = [
			{
				type: "ContainersReady",
				status: "False",
				reason: "PodFailed",
				lastTransitionTime: "2026-06-12T18:01:00.000Z",
			},
			{
				type: "Ready",
				status: "False",
				reason: "PodFailed",
				lastTransitionTime: "2026-06-12T18:01:00.000Z",
			},
			{
				type: "PodReadyToStartContainers",
				status: "False",
				lastTransitionTime: "2026-06-12T18:01:00.000Z",
			},
		];
		const containers: ContainerStatusRow[] = [
			{
				name: "fail",
				type: "container",
				ready: false,
				restartCount: 0,
				state: "terminated",
				reason: "Error",
				exitCode: 42,
				finishedAt: "2026-06-12T18:01:00.000Z",
			},
		];
		const failedPod = resource({
			name: "issue-143-failure-control-99jns",
			status: "Failed",
			ready: "False",
			health: "degraded",
		});

		const signals = buildIncidentSignals(failedPod, conditions, [], containers);
		expect(signals.map((signal) => signal.id)).toEqual([
			"container:fail:terminated",
			"condition:ContainersReady",
			"condition:Ready",
			"condition:PodReadyToStartContainers",
			"status",
			"ready",
		]);
		expect(signals[0]).toMatchObject({
			label: "fail terminated",
			source: "container",
			tone: "error",
		});
		expect(signals[0].value).toContain("exit 42");

		const timeline = buildIncidentTimeline({
			resource: failedPod,
			conditions,
			events: [],
			containers,
		});
		const titles = timeline.map((item) => item.title);
		expect(titles).toContain("fail terminated");
		expect(titles).not.toContain("Status Failed");
		expect(titles).not.toContain("Ready False");
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

	test("Svelte resource detail renders events in incident priority order", () => {
		const source = svelteDetailSource();

		expect(source).toContain("sortIncidentEvents(eventsQuery.data ?? [])");
		expect(source).toContain("filterResourceEvents(sortedEvents, eventTypeFilter, eventSearch)");
		expect(source).toContain("{#each visibleEvents as event");
		expect(source).toContain("{sourceLabel(event)}");
		expect(source).toContain("{event.namespace ?? \"cluster\"}");
		expect(source).toContain("function formatEventCompactTime(event: ResourceEventSummary)");
		expect(source).toContain('formatExactTimestamp(event.lastSeenAt, timestampTimezone, "millisecond")');
		expect(source).toContain('formatExactTimeOnly(event.lastSeenAt, timestampTimezone, "second")');
		expect(source).toContain("{formatEventCompactTime(event)}");
		expect(source).toContain("{eventAgeLabel(event)}");
		expect(source).toContain("datetime={event.lastSeenAt}");
		expect(source).toContain("title={formatEventExactTime(event)}");
	});

	test("Svelte resource detail wires incident summary and timeline helpers", () => {
		const source = svelteDetailSource();

		expect(source).toContain("buildIncidentSignals(");
		expect(source).toContain("buildIncidentTimeline({");
		expect(source).toContain("Incident summary");
		expect(source).toContain("No incident timeline entries for this resource.");
	});

	test("Svelte resource detail orders conditions before curated metadata", () => {
		const source = svelteDetailSource();

		expect(source).toContain("buildCuratedMetadata(detailsQuery.data?.metadata ?? {}, detailResource)");
		expect(source).toContain(">Conditions</div>");
		expect(source).toContain(">Metadata</div>");
		expect(source).not.toContain(">Diagnostics</div>");
		expect(source.indexOf(">Conditions</div>")).toBeLessThan(
			source.indexOf(">Metadata</div>"),
		);
		expect(source).not.toContain("diagnosticBadgeMetadataRows");
		expect(source).not.toContain("metadataBadgeRows");
		expect(source).not.toContain("hasOwnership");
		expect(source).not.toContain('<Badge variant="outline">Owner {detailResource.ownerRef}</Badge>');
	});

	test("Svelte resource detail formats metadata and timestamps", () => {
		const source = svelteDetailSource();

		expect(source).toContain("function metadataBadgeStyle(key: string)");
		expect(source).toContain("visibleMetadataBadges(curatedMetadata.labels, metadataLabelsExpanded)");
		expect(source).toContain("visibleMetadataBadges(curatedMetadata.annotations, metadataAnnotationsExpanded)");
		expect(source).toContain("<details class=\"rounded-md border bg-background/30\">");
		expect(source).toContain("Advanced metadata");
		expect(source).toContain("Show {visibleMetadataLabels.hiddenCount} more");
		expect(source).toContain("Show {visibleMetadataAnnotations.hiddenCount} more");
		expect(source).toContain('row.label === "Helm" && onOpenHelmRelease');
		expect(source).toContain("onOpenHelmRelease?.(row.value, detailResource.namespace)");
		expect(source).toContain("formatFullTimestamp(row.value)");
		expect(source).toContain("title={formatFullTimestamp(line.timestamp)}");
		expect(source).toContain("<TableHead>Last transition</TableHead>");
		expect(source).toContain("datetime={condition.lastTransitionTime}");
		expect(source).toContain("formatFullTimestamp(condition.lastTransitionTime)");
		expect(source).toContain("<TableHead>Last finished</TableHead>");
		expect(source).toContain("datetime={container.lastFinishedAt}");
		expect(source).toContain("formatFullTimestamp(container.lastFinishedAt)");
		expect(source).not.toContain("Sampled {formatLogTime(detailResource.metrics.sampledAt)}");
	});

	test("Svelte resource detail cancels stale backend requests", () => {
		const source = svelteDetailSource();

		expect(source).toContain('createCancelScope("resource-details", detailsQueryKey)');
		expect(source).toContain('createCancelScope("resource-yaml", yamlQueryKey)');
		expect(source).toContain('createCancelScope("resource-events", eventsQueryKey)');
		expect(source).toContain('createCancellableRequest(detailsCancelScope, "details")');
		expect(source).toContain('createCancellableRequest(yamlCancelScope, "yaml")');
		expect(source).toContain('createCancellableRequest(eventsCancelScope, "events")');
		expect(source).toContain("cancelBackendRequests(client, cancelScope)");
		expect(source).toContain("queryKeys.resourceDetails(");
		expect(source).toContain("queryKeys.resourceYaml(");
		expect(source).toContain("queryKeys.resourceEvents(");
		expect(source).toContain('"detail.details.cancel"');
		expect(source).toContain('"detail.yaml.cancel"');
		expect(source).toContain('"detail.events.cancel"');
	});

	test("Svelte resource detail honors YAML defaults and exposes YAML mode controls", () => {
		const source = svelteDetailSource();

		expect(source).toContain("getSettingsSnapshot().yamlViewModeDefault");
		expect(source).toContain("getSettingsSnapshot().yamlEncodingDefault");
		expect(source).toContain("function setYamlViewMode(value: string)");
		expect(source).toContain("function setYamlEncoding(value: string)");
		expect(source).toContain('aria-label="YAML shape"');
		expect(source).toContain('aria-label="YAML encoding"');
		expect(source).toContain('value={yamlEditing ? "applyClean" : yamlViewMode}');
		expect(source).toContain(
			'<SelectValue>{yamlEditing || yamlViewMode === "applyClean" ? "Apply clean" : "Kubectl view"}</SelectValue>',
		);
		expect(source).toContain(
			'<SelectValue>{yamlEncoding === "kyaml" ? "KYAML" : "YAML"}</SelectValue>',
		);
		expect(source).toContain(
			'<SelectItem value="kubectl" label="Kubectl view">Kubectl view</SelectItem>',
		);
		expect(source).toContain(
			'<SelectItem value="applyClean" label="Apply clean">Apply clean</SelectItem>',
		);
		expect(source).toContain('<SelectItem value="yaml" label="YAML">YAML</SelectItem>');
		expect(source).toContain('<SelectItem value="kyaml" label="KYAML">KYAML</SelectItem>');
		expect(source).toContain('onclick={() => void previewYamlApply()}');
		expect(source).toContain(
			'title={!yamlPreview ? "Run a dry run before applying." : undefined}',
		);
		expect(source).toContain(
			'aria-describedby={!yamlPreview ? "yaml-apply-disabled-tooltip" : undefined}',
		);
		expect(source).toContain('role="tooltip"');
		expect(source).toContain("group-hover:block");
		expect(source).toContain(
			'<div class="overflow-x-auto py-1 font-mono text-xs leading-relaxed">',
		);
		expect(source).not.toContain(
			'<pre class="overflow-x-auto p-0 font-mono text-xs leading-relaxed">',
		);
		expect(source).toContain('<YamlCodeEditor value={yamlText} minHeight="520px" />');
		expect(source.indexOf("Dry-run diff")).toBeLessThan(
			source.indexOf("bind:value={yamlDraft}"),
		);
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
		]);
		expect(timeline.find((item) => item.source === "event")?.detail).toContain(
			"3 repeats",
		);
		expect(timeline.find((item) => item.source === "log")?.detail).toBe(
			"request failed",
		);
		expect(timeline.map((item) => item.title)).not.toContain("Status Pending");
		expect(timeline.map((item) => item.title)).not.toContain("Ready False");
	});

	test("incident timeline timestamps terminated containers at finish time", () => {
		const timeline = buildIncidentTimeline({
			resource: resource(),
			conditions: [],
			events: [],
			containers: [
				{
					name: "worker",
					ready: false,
					restartCount: 0,
					state: "terminated",
					exitCode: 1,
					startedAt: "2026-05-19T09:00:00.000Z",
					finishedAt: "2026-05-19T09:45:00.000Z",
				},
			],
		});

		expect(timeline).toHaveLength(1);
		expect(timeline[0]).toMatchObject({
			source: "container",
			timestamp: "2026-05-19T09:45:00.000Z",
		});
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

	test("incident timeline keeps same-time warning events with distinct messages", () => {
		const timeline = buildIncidentTimeline({
			resource: resource(),
			conditions: [],
			events: [
				event({
					eventType: "Warning",
					reason: "FailedMount",
					message: "secret missing",
					lastSeenAt: "2026-05-19T10:00:00.000Z",
				}),
				event({
					eventType: "Warning",
					reason: "FailedMount",
					message: "configmap missing",
					lastSeenAt: "2026-05-19T10:00:00.000Z",
				}),
			],
		});

		expect(timeline.map((item) => item.detail)).toEqual([
			"configmap missing",
			"secret missing",
		]);
	});

	test("incident timeline ignores malformed log timestamps for latest sample", () => {
		const timeline = buildIncidentTimeline({
			resource: resource(),
			conditions: [],
			events: [],
			logLines: [
				parseLogLine('time="not-a-time" malformed', 0),
				parseLogLine("2026-05-19T10:00:00Z valid", 1),
			],
		});

		expect(timeline).toHaveLength(1);
		expect(timeline[0]).toMatchObject({
			source: "log",
			detail: "valid",
			timestamp: "2026-05-19T10:00:00Z",
		});
	});

	test("incident timeline skips ready false for succeeded pods", () => {
		const timeline = buildIncidentTimeline({
			resource: resource({ status: "Succeeded", ready: "False" }),
			conditions: [],
			events: [],
		});

		expect(timeline).toEqual([]);
	});

	test("incident timeline skips completed readiness conditions for succeeded pods", () => {
		const timeline = buildIncidentTimeline({
			resource: resource({ status: "Succeeded", ready: "False" }),
			conditions: [
				{ type: "Ready", status: "False", reason: "PodCompleted" },
				{ type: "ContainersReady", status: "False", reason: "PodCompleted" },
				{ type: "PodReadyToStartContainers", status: "False" },
			],
			events: [],
		});

		expect(timeline).toEqual([]);
	});

	test("incident timeline keeps disruption target conditions", () => {
		const timeline = buildIncidentTimeline({
			resource: resource(),
			conditions: [
				{
					type: "DisruptionTarget",
					status: "True",
					reason: "EvictionByEvictionAPI",
					lastTransitionTime: "2026-05-19T09:50:00.000Z",
				},
			],
			events: [],
		});

		expect(timeline).toHaveLength(1);
		expect(timeline[0]).toMatchObject({
			source: "condition",
			tone: "info",
			timestamp: "2026-05-19T09:50:00.000Z",
		});
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

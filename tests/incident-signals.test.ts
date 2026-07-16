import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import type { ResourceEventSummary, ResourceSummary } from "../src/lib/types";
import {
	buildIncidentSignals,
	incidentSignalCardClassName,
	shouldFetchResourceEvents,
} from "../src/features/resource-detail/helpers";

describe("incident signal helpers", () => {
  const resource: ResourceSummary = {
    cluster: "kind-prod",
    kind: "Pod",
    name: "api-0",
    namespace: "payments",
    age: "3h",
  };

  test("summarizes factual selected-resource incident signals", () => {
    const conditions = [
      {
        type: "Ready",
        status: "False",
        reason: "ContainersNotReady",
        message: "containers with unready status",
        lastTransitionTime: "2026-05-17T11:55:00Z",
      },
      { type: "Initialized", status: "True" },
    ];
    const events: ResourceEventSummary[] = [
      {
        eventType: "Warning",
        reason: "BackOff",
        message: "Back-off restarting failed container",
        count: 4,
        lastSeen: "2m",
        source: "kubelet",
        namespace: "payments",
      },
      {
        eventType: "Normal",
        reason: "Pulled",
        message: "Container image already present",
        count: 1,
        lastSeen: "4m",
        source: "kubelet",
        namespace: "payments",
      },
    ];

    expect(
      buildIncidentSignals(
        { ...resource, status: "Failed", ready: "False", restarts: 3 },
        conditions,
        events,
      ),
    ).toEqual([
      {
        id: "events:warnings",
        label: "Warning events",
        value: "1 warning reason · 4 repeats",
        tone: "warning",
        source: "event",
      },
      {
        id: "restarts",
        label: "Restarts",
        value: "3",
        valueParts: undefined,
        tone: "warning",
        source: "status",
      },
      {
        id: "condition:Ready",
        label: "Condition",
        value: "Ready=False · ContainersNotReady · since 2026-05-17T11:55:00Z",
        valueParts: [
          { kind: "text", text: "Ready=False" },
          { kind: "text", text: " · ContainersNotReady" },
          { kind: "text", text: " · since " },
          { kind: "timestamp", value: "2026-05-17T11:55:00Z" },
        ],
        tone: "error",
        source: "condition",
      },
      {
        id: "status",
        label: "Status",
        value: "Failed",
        tone: "error",
        source: "status",
      },
      {
        id: "ready",
        label: "Ready",
        value: "False",
        tone: "error",
        source: "status",
      },
    ]);
  });

	test("returns no incident signals for healthy resources with no warning events", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 0 },
				[{ type: "Ready", status: "True" }],
				[],
      ),
		).toEqual([]);
	});

	test("does not promote old clean restarts on currently ready containers", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "applicationset-controller",
						ready: true,
						restartCount: 1,
						state: "running",
						startedAt: "2026-05-13T12:28:58Z",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "2026-05-13T12:25:53Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([]);
	});

	test("does not promote old clean completed init container restarts", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "copyutil",
						type: "init",
						ready: false,
						restartCount: 1,
						state: "terminated",
						reason: "Completed",
						exitCode: 0,
						finishedAt: "2026-05-13T12:25:53Z",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "2026-05-13T12:25:53Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([]);
	});

	test("does not promote clean restarts with unknown timestamps", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "sidecar",
						ready: true,
						restartCount: 1,
						state: "running",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([]);

		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "sidecar",
						ready: true,
						restartCount: 1,
						state: "running",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "not-a-timestamp",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([]);
	});

	test("keeps recent clean restarts actionable", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "worker",
						ready: true,
						restartCount: 1,
						state: "running",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "2026-05-17T11:55:00Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([
			{
				id: "restarts",
				label: "Restarts",
				value: "worker restarted 1 time · Completed · exit 0 · finished 2026-05-17T11:55:00Z",
				valueParts: [
					{ kind: "text", text: "worker restarted 1 time" },
					{ kind: "text", text: " · Completed" },
					{ kind: "text", text: " · exit 0" },
					{ kind: "text", text: " · finished " },
					{ kind: "timestamp", value: "2026-05-17T11:55:00Z" },
				],
				tone: "warning",
				source: "status",
			},
		]);
	});

	test("falls back to restart signals when restart history is unknown", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "sidecar",
						ready: true,
						restartCount: 1,
						state: "running",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([
			{
				id: "restarts",
				label: "Restarts",
				value: "sidecar restarted 1 time",
				valueParts: [{ kind: "text", text: "sidecar restarted 1 time" }],
				tone: "warning",
				source: "status",
			},
		]);
	});

	test("does not fall back to raw restart counts when container context is pending", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[],
			),
		).toEqual([]);
	});

	test("treats succeeded completed pods as historical context, not active incidents", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Succeeded", ready: "False", restarts: 1 },
				[
					{
						type: "DisruptionTarget",
						status: "True",
						reason: "TerminationByKubelet",
						message: "Pod was terminated in response to imminent node shutdown.",
						lastTransitionTime: "2026-05-13T11:54:59Z",
					},
					{
						type: "PodReadyToStartContainers",
						status: "False",
						lastTransitionTime: "2026-05-13T11:54:58Z",
					},
					{
						type: "Ready",
						status: "False",
						reason: "PodCompleted",
						lastTransitionTime: "2026-05-13T11:54:58Z",
					},
					{
						type: "ContainersReady",
						status: "False",
						reason: "PodCompleted",
						lastTransitionTime: "2026-05-13T11:54:58Z",
					},
				],
				[],
				[
					{
						name: "redis",
						ready: false,
						restartCount: 1,
						state: "terminated",
						reason: "Completed",
						exitCode: 0,
						finishedAt: "2026-05-13T11:54:44Z",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "2026-05-13T11:54:44Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([
			{
				id: "condition:DisruptionTarget",
				label: "Condition",
				value: "DisruptionTarget=True · TerminationByKubelet · since 2026-05-13T11:54:59Z",
				valueParts: [
					{ kind: "text", text: "DisruptionTarget=True" },
					{ kind: "text", text: " · TerminationByKubelet" },
					{ kind: "text", text: " · since " },
					{ kind: "timestamp", value: "2026-05-13T11:54:59Z" },
				],
				tone: "info",
				source: "condition",
			},
		]);
	});

	test("keeps restart signals for recent or unclean container restarts", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 1 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "api",
						ready: true,
						restartCount: 1,
						state: "running",
						lastState: "terminated",
						lastReason: "Error",
						lastExitCode: 1,
						lastFinishedAt: "2026-05-17T11:55:00Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([
			{
				id: "restarts",
				label: "Restarts",
				value: "api restarted 1 time · Error · exit 1 · finished 2026-05-17T11:55:00Z",
				valueParts: [
					{ kind: "text", text: "api restarted 1 time" },
					{ kind: "text", text: " · Error" },
					{ kind: "text", text: " · exit 1" },
					{ kind: "text", text: " · finished " },
					{ kind: "timestamp", value: "2026-05-17T11:55:00Z" },
				],
				tone: "warning",
				source: "status",
			},
		]);
	});

	test("derives restart severity from actionable restarts only", () => {
		expect(
			buildIncidentSignals(
				{ ...resource, status: "Running", ready: "True", restarts: 8 },
				[{ type: "Ready", status: "True" }],
				[],
				[
					{
						name: "api",
						ready: true,
						restartCount: 1,
						state: "running",
						lastState: "terminated",
						lastReason: "Error",
						lastExitCode: 1,
						lastFinishedAt: "2026-05-17T11:55:00Z",
					},
					{
						name: "sidecar",
						ready: true,
						restartCount: 7,
						state: "running",
						lastState: "terminated",
						lastReason: "Completed",
						lastExitCode: 0,
						lastFinishedAt: "2026-05-13T12:25:53Z",
					},
				],
				{ now: new Date("2026-05-17T12:00:00Z") },
			),
		).toEqual([
			{
				id: "restarts",
				label: "Restarts",
				value: "api restarted 1 time · Error · exit 1 · finished 2026-05-17T11:55:00Z",
				valueParts: [
					{ kind: "text", text: "api restarted 1 time" },
					{ kind: "text", text: " · Error" },
					{ kind: "text", text: " · exit 1" },
					{ kind: "text", text: " · finished " },
					{ kind: "timestamp", value: "2026-05-17T11:55:00Z" },
				],
				tone: "warning",
				source: "status",
			},
		]);
	});

	test("classifies waiting containers before generic not-ready containers", () => {
		const source = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);
		const waitingIndex = source.indexOf('container.state === "waiting"');
		const notReadyIndex = source.indexOf("container.ready === false");

		expect(waitingIndex).toBeGreaterThanOrEqual(0);
		expect(notReadyIndex).toBeGreaterThanOrEqual(0);
		expect(waitingIndex).toBeLessThan(notReadyIndex);
	});

	test("falls back to summary restarts only after pod details are unavailable", () => {
		const source = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);

		expect(source).toContain("detailsQuery.data?.summary");
		expect(source).toContain('resource.kind === "Pod"');
		expect(source).toContain("containerRows");
		expect(source).not.toContain(
			'resource.kind === "Pod" || details ? containerRows : undefined',
		);
	});

	test("renders signal timestamps through the shared timestamp formatter", () => {
		const source = readFileSync(
			"src/features/resource-detail/DetailsTab.svelte",
			"utf8",
		);

		expect(source).toContain("signalValueParts(signal)");
		expect(source).toContain("formatFullTimestamp(part.value)");
		expect(source).not.toContain("{signal.value}");
	});

	test("renders detail timeline timestamps with millisecond precision", () => {
		const source = readFileSync(
			"src/features/resource-detail/DetailsTab.svelte",
			"utf8",
		);

		expect(source).toContain("formatFullTimestamp(item.timestamp)");
	});

	test("renders metadata creation timestamps through the shared timestamp formatter", () => {
		const source = readFileSync(
			"src/features/resource-detail/DetailsTab.svelte",
			"utf8",
		);

		expect(source).toContain('row.label === "Created"');
		expect(source).toContain(
			"<time datetime={row.value} title={formatFullTimestamp(row.value)}>",
		);
	});

	test("keeps GitOps age tooltip backed by creation timestamps", () => {
		const source = readFileSync(
			"src/features/gitops/surfaceTooltips.ts",
			"utf8",
		);

		expect(source).toContain("gitOpsSelectionAgeTooltip");
		expect(source).toContain("selection.item.createdAt ?? null");
	});

  test("treats crash loop and image pull states as error incident signals", () => {
    expect(
      buildIncidentSignals(
        { ...resource, status: "CrashLoopBackOff" },
        [],
        [],
      ),
    ).toEqual([
      {
        id: "status",
        label: "Status",
        value: "CrashLoopBackOff",
        tone: "error",
        source: "status",
      },
    ]);

    expect(
      buildIncidentSignals(
        { ...resource, status: "ImagePullBackOff" },
        [],
        [],
      )[0]?.tone,
    ).toBe("error");
  });

  test("fetches selected resource events whenever a resource identity is available", () => {
    expect(shouldFetchResourceEvents(resource)).toBe(true);
    expect(shouldFetchResourceEvents({ ...resource, name: "" })).toBe(false);
  });

  test("maps incident signal severity to scannable card accents", () => {
    expect(incidentSignalCardClassName("error")).toContain("border-l-red-500");
    expect(incidentSignalCardClassName("warning")).toContain("border-l-amber-500");
    expect(incidentSignalCardClassName("info")).toContain("border-l-sky-500");
    expect(incidentSignalCardClassName("neutral")).toContain("border-l-muted");
  });
});

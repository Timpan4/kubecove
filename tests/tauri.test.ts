import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import type {
  DiscoveredResourceKind,
  ResourceEventSummary,
  ResourceSummary,
} from "../src/lib/types";
import {
	buildIncidentSignals,
	getContainerStatusRows,
	getConditionRows,
	incidentSignalCardClassName,
	shouldFetchResourceEvents,
	shouldFetchResourceDetails,
} from "../src/features/resource-detail/helpers";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	describeResourceScope,
	filterResources,
	filterResourcesByHealth,
	formatResourceGroupLabel,
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
	formatResourceTypeGroupLabel,
	sortedRows,
	tableTooltipText,
	uniqueArgoApps,
	watchKeysFromFetchKeys,
} from "../src/features/resources/helpers";
import {
	emptyStateMessage,
	resolveTreeScope,
	type TreeNodeId,
} from "../src/lib/tree-nav";

describe("shouldFetchResourceDetails", () => {
  const resource = {
    cluster: "minikube",
    kind: "Pod",
    name: "test-pod",
    namespace: "default",
    age: "1m",
  };

  test("fetches full details when a valid resource is selected", () => {
    expect(shouldFetchResourceDetails(resource)).toBe(true);
  });

  test("does not fetch details until a complete resource identity exists", () => {
    expect(shouldFetchResourceDetails({ ...resource, name: "" })).toBe(false);
  });
});

describe("getConditionRows", () => {
  test("extracts readable Kubernetes condition rows from status", () => {
    expect(
      getConditionRows({
        conditions: [
          {
            type: "Ready",
            status: "False",
            reason: "ContainersNotReady",
            message: "containers with unready status",
            lastTransitionTime: "2026-05-17T11:55:00Z",
          },
        ],
      }),
    ).toEqual([
      {
        type: "Ready",
        status: "False",
        reason: "ContainersNotReady",
        message: "containers with unready status",
        lastTransitionTime: "2026-05-17T11:55:00Z",
      },
    ]);
  });

  test("returns an empty list when status conditions are absent", () => {
    expect(getConditionRows({ phase: "Running" })).toEqual([]);
  });
});

describe("getContainerStatusRows", () => {
	test("extracts container restart context with timestamps", () => {
		expect(
			getContainerStatusRows({
				containerStatuses: [
					{
						name: "api",
						ready: true,
						restartCount: 1,
						state: { running: { startedAt: "2026-05-17T11:56:00Z" } },
						lastState: {
							terminated: {
								reason: "Error",
								exitCode: 1,
								startedAt: "2026-05-17T11:45:00Z",
								finishedAt: "2026-05-17T11:55:00Z",
							},
						},
					},
				],
			}),
		).toEqual([
			{
				name: "api",
				type: "container",
				ready: true,
				restartCount: 1,
				state: "running",
				startedAt: "2026-05-17T11:56:00Z",
				lastState: "terminated",
				lastReason: "Error",
				lastExitCode: 1,
				lastStartedAt: "2026-05-17T11:45:00Z",
				lastFinishedAt: "2026-05-17T11:55:00Z",
			},
		]);
	});
});

describe("resource browser presentation helpers", () => {
  const baseResource: ResourceSummary = {
    cluster: "kind-prod",
    kind: "Pod",
    name: "api-0",
    namespace: "payments",
    age: "3h",
  };
  const widgetKind: DiscoveredResourceKind = {
    group: "example.com",
    version: "v1",
    apiVersion: "example.com/v1",
    kind: "Widget",
    plural: "widgets",
    namespaced: true,
  };
  const clusterWidgetKind: DiscoveredResourceKind = {
    ...widgetKind,
    kind: "ClusterWidget",
    plural: "clusterwidgets",
    namespaced: false,
  };

  test("summarizes resource health for the active filter", () => {
    const summary = buildResourceHealthSummary([
      { ...baseResource, name: "api-0", status: "Running", ready: "True", restarts: 0 },
      { ...baseResource, name: "worker-0", status: "Pending", restarts: 0 },
      { ...baseResource, name: "job-0", status: "Failed", ready: "False", restarts: 1 },
    ]);

    expect(summary).toEqual({
      total: 3,
      healthy: 1,
      attention: 1,
      degraded: 1,
      restarted: 1,
    });
  });

  test("filters resources by transient health filter", () => {
    const resources: ResourceSummary[] = [
      { ...baseResource, name: "api-0", status: "Running", ready: "True", restarts: 0 },
      { ...baseResource, name: "worker-0", status: "Pending", restarts: 0 },
      { ...baseResource, name: "job-0", status: "Failed", ready: "False", restarts: 1 },
      { ...baseResource, name: "cache-0", status: "Running", ready: "True", restarts: 2 },
    ];

    expect(filterResourcesByHealth(resources, "all")).toEqual(resources);
    expect(filterResourcesByHealth(resources, "attention").map((r) => r.name)).toEqual(["worker-0", "cache-0"]);
    expect(filterResourcesByHealth(resources, "degraded").map((r) => r.name)).toEqual(["job-0"]);
    expect(filterResourcesByHealth(resources, "restarted").map((r) => r.name)).toEqual(["job-0", "cache-0"]);
  });

  test("builds fetch keys for namespaced and cluster-scoped kinds", () => {
    expect(buildFetchKeys(["default", "payments"], ["Pod", "Node"])).toEqual([
      { kind: "Pod", namespace: "default" },
      { kind: "Pod", namespace: "payments" },
      { kind: "Node", namespace: undefined },
    ]);
  });

  test("builds fetch keys for discovered resource kinds", () => {
    expect(buildFetchKeys([], [widgetKind])).toEqual([
      { kind: widgetKind, namespace: undefined },
    ]);
    expect(buildFetchKeys(["default"], [widgetKind, clusterWidgetKind])).toEqual([
      { kind: widgetKind, namespace: "default" },
      { kind: clusterWidgetKind, namespace: undefined },
    ]);
  });

  test("builds realtime watch keys for typed and discovered resources", () => {
    expect(watchKeysFromFetchKeys([
      { kind: "Pod", namespace: "default" },
      { kind: widgetKind, namespace: "apps" },
    ])).toEqual([
      { resourceKind: { kind: "Pod" }, namespace: "default" },
      {
        resourceKind: {
          kind: "Widget",
          group: "example.com",
          version: "v1",
          apiVersion: "example.com/v1",
          plural: "widgets",
          namespaced: true,
        },
        namespace: "apps",
      },
    ]);
  });

  test("filters resources by search text and Argo app", () => {
    const resources: ResourceSummary[] = [
      { ...baseResource, name: "api-0", ownerRef: "api", argoApp: "payments" },
      { ...baseResource, name: "worker-0", helmRelease: "jobs", argoApp: "batch" },
      { ...baseResource, name: "config", kind: "ConfigMap", namespace: "platform" },
    ];

    expect(filterResources(resources, "jobs", "")).toEqual([resources[1]]);
    expect(filterResources(resources, "api", "payments")).toEqual([resources[0]]);
    expect(filterResources(resources, "api", "batch")).toEqual([]);
  });

  test("filters dynamic resources by discovered metadata", () => {
    const resource: ResourceSummary = {
      ...baseResource,
      kind: "Widget",
      apiVersion: "example.com/v1",
      group: "example.com",
      plural: "widgets",
      dynamic: true,
    };

    expect(filterResources([resource], "widgets", "")).toEqual([resource]);
    expect(filterResources([resource], "example.com", "")).toEqual([resource]);
    expect(filterResources([resource], "apps/v1", "")).toEqual([]);
  });

  test("sorts rows using table sorting state", () => {
    const resources: ResourceSummary[] = [
      { ...baseResource, name: "worker", restarts: 10 },
      { ...baseResource, name: "api", restarts: 2 },
    ];

    expect(sortedRows(resources, [{ id: "name", desc: false }]).map((r) => r.name)).toEqual(["api", "worker"]);
    expect(sortedRows(resources, [{ id: "name", desc: true }]).map((r) => r.name)).toEqual(["worker", "api"]);
    expect(sortedRows(resources, [{ id: "restarts", desc: false }]).map((r) => r.restarts)).toEqual([2, 10]);
  });

  test("describes namespace-first scope in the table header", () => {
    expect(describeResourceScope("kind-prod", ["payments"], ["Pod", "Service"], "argocd")).toEqual([
      { label: "Context", value: "kind-prod" },
      { label: "Namespace", value: "payments" },
      { label: "Kinds", value: "Pod, Service" },
      { label: "Argo app", value: "argocd" },
    ]);
  });

  test("uses readable group labels for Argo-managed and unmanaged resources", () => {
    expect(formatResourceGroupLabel({ ...baseResource, argoApp: "argocd" })).toBe("Managed by Argo app: argocd");
    expect(formatResourceGroupLabel({ ...baseResource, ownerRef: "api" })).toBe("Owned by: api");
    expect(formatResourceGroupLabel(baseResource)).toBe("Unmanaged resources");
  });

  test("returns sorted unique Argo apps", () => {
    expect(
      uniqueArgoApps([
        { ...baseResource, argoApp: "zeta" },
        { ...baseResource, argoApp: "alpha" },
        { ...baseResource, argoApp: "zeta" },
        baseResource,
      ]),
    ).toEqual(["alpha", "zeta"]);
  });

  test("uses readable type subgroup labels", () => {
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Pod" })).toBe("Pods");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Ingress" })).toBe("Ingresses");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "ConfigMap" })).toBe("ConfigMaps");
  });

  test("builds stable collapse keys for app and type groups", () => {
    const resource = { ...baseResource, kind: "ConfigMap", argoApp: "argocd" };

    expect(resourceGroupCollapseKey(resource)).toBe("group:Managed by Argo app: argocd");
    expect(resourceTypeGroupCollapseKey(resource)).toBe("group:Managed by Argo app: argocd::type:ConfigMaps");
  });

  test("normalizes tooltip values for table display", () => {
    expect(tableTooltipText("argocd-server-7886b899c8-l5lqd")).toBe("argocd-server-7886b899c8-l5lqd");
    expect(tableTooltipText(null)).toBe("—");
    expect(tableTooltipText("")).toBe("—");
  });

  test("centers restart count badges in the resource table", () => {
    const cellsSource = readFileSync("src/features/resources/cells.tsx", "utf8");
    const columnsSource = readFileSync("src/features/resources/columns.tsx", "utf8");

    expect(cellsSource).toContain("export function RestartsCell");
    expect(cellsSource).toContain("justify-center");
    expect(columnsSource).toContain("RestartsCell");
  });

  test("uses pointer cursors for enabled shared interactive controls", () => {
    const controlFiles = [
      "src/components/ui/button.tsx",
      "src/components/ui/tabs.tsx",
      "src/components/ui/select.tsx",
      "src/components/ui/checkbox.tsx",
    ];

    for (const file of controlFiles) {
      expect(readFileSync(file, "utf8")).toContain("cursor-pointer");
    }
  });

  test("keeps the ownership map mounted as the primary resource view", () => {
    const appSource = readFileSync("src/App.tsx", "utf8");
    const listSource = readFileSync("src/features/resources/ResourceList.tsx", "utf8");
    const layoutSource = readFileSync("src/features/resources/ResourceMapTableLayout.tsx", "utf8");

    expect(listSource).toContain("<ResourceMapTableLayout");
    expect(layoutSource).toContain("tablePanelOpen");
    expect(layoutSource).toContain("hasActiveSelection");
    expect(layoutSource).toContain("mapHeightClassName");
    expect(appSource).toContain("selectedResource={selectedResource}");
    expect(listSource).toContain("activeSelectedResourceKey");
    expect(layoutSource).toContain("xl:grid-cols-[minmax(620px,1fr)_minmax(420px,0.82fr)]");
    expect(layoutSource).toContain("h-[360px]");
    expect(layoutSource).toContain("h-[560px]");
    expect(layoutSource).toContain("<OwnershipMap");
    expect(layoutSource).toContain("Hide table");
    expect(listSource).not.toContain('resourceView === "map"');
    expect(listSource).not.toContain('resourceView === "table"');
  });
});

describe("sidebar source safeguards", () => {
	test("avoids unsafe returns in namespace cleanup", () => {
		const source = readFileSync("src/components/NamespaceList.tsx", "utf8");

		expect(source).not.toContain(
			"finally {\n\t\t\tif (requestSeq !== requestSeqRef.current) return;",
		);
		expect(source).toContain("if (requestSeq === requestSeqRef.current)");
	});

	test("uses non-submit button types for sidebar controls", () => {
		const layoutSource = readFileSync(
			"src/components/ui/sidebar-layout.tsx",
			"utf8",
		);
		const menuSource = readFileSync(
			"src/components/ui/sidebar-menu.tsx",
			"utf8",
		);

		expect(layoutSource).toContain('type="button"');
		expect(layoutSource).toContain('type={asChild ? undefined : "button"}');
		expect(menuSource.match(/type=\{asChild \? undefined : "button"\}/g))
			.toHaveLength(2);
	});

	test("ignores the sidebar keyboard shortcut inside editable controls", () => {
		const source = readFileSync(
			"src/components/ui/sidebar-provider.tsx",
			"utf8",
		);

		expect(source).toContain("const isEditable");
		expect(source).toContain("HTMLInputElement");
		expect(source).toContain("HTMLTextAreaElement");
		expect(source).toContain("HTMLSelectElement");
		expect(source).toContain('getAttribute("role") === "textbox"');
	});
});

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
      {
        id: "restarts",
        label: "Restarts",
        value: "3",
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
        id: "events:warnings",
        label: "Warning events",
        value: "1 warning reason · 4 repeats",
        tone: "warning",
        source: "event",
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
			"src/features/resource-detail/DetailsTab.tsx",
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
			"src/features/resource-detail/DetailsTab.tsx",
			"utf8",
		);

		expect(source).toContain(
			'resource.kind === "Pod" && (details || podDetailsLoading)',
		);
		expect(source).toContain("details || podDetailsLoading");
		expect(source).toContain("? containerRows");
		expect(source).toContain(": undefined");
		expect(source).not.toContain(
			'resource.kind === "Pod" || details ? containerRows : undefined',
		);
	});

	test("renders signal timestamps through the shared timestamp formatter", () => {
		const detailsSource = readFileSync(
			"src/features/resource-detail/DetailsTab.tsx",
			"utf8",
		);
		const valueSource = readFileSync(
			"src/features/resource-detail/IncidentSignalValue.tsx",
			"utf8",
		);

		expect(detailsSource).toContain("<IncidentSignalValue signal={signal} />");
		expect(valueSource).toContain("ExactTimestampText");
		expect(valueSource).toContain("valueParts");
		expect(valueSource).not.toContain("{signal.value}");
	});

	test("renders metadata creation timestamps through the shared timestamp formatter", () => {
		const source = readFileSync(
			"src/features/resource-detail/DetailsTab.tsx",
			"utf8",
		);

		expect(source).toContain('name === "Created"');
		expect(source).toContain("<ExactTimestampText value={value} />");
	});

	test("renders Argo detail metadata creation timestamps through the shared timestamp formatter", () => {
		const source = readFileSync(
			"src/features/argo/ArgoDetailShared.tsx",
			"utf8",
		);

		expect(source).toContain('key === "Created"');
		expect(source).toContain("<ExactTimestampText value={value} />");
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

describe("tree navigation scope helpers", () => {
  test("resolves null scope to no query", () => {
    expect(resolveTreeScope(null)).toEqual({
      section: null,
      namespace: null,
      group: null,
      kinds: [],
      clusterScoped: false,
      argoMode: false,
    });
  });

  test("resolves namespace nodes to all namespaced supported kinds", () => {
    const scope = resolveTreeScope({ type: "namespace", section: "namespaces", namespace: "payments" });
    expect(scope.namespace).toBe("payments");
    expect(scope.clusterScoped).toBe(false);
    expect(scope.argoMode).toBe(false);
    expect(scope.kinds).toContain("Pod");
    expect(scope.kinds).not.toContain("Node");
  });

  test("resolves group and kind nodes", () => {
    expect(
      resolveTreeScope({ type: "group", section: "namespaces", namespace: "default", group: "Workloads" }),
    ).toMatchObject({
      section: "namespaces",
      namespace: "default",
      group: "Workloads",
      clusterScoped: false,
    });

    expect(
      resolveTreeScope({ type: "kind", section: "namespaces", namespace: "default", group: "Workloads", kind: "Deployment" }),
    ).toMatchObject({
      section: "namespaces",
      namespace: "default",
      group: "Workloads",
      kinds: ["Deployment"],
      clusterScoped: false,
    });
  });

  test("resolves cluster-scoped and Argo nodes", () => {
    expect(
      resolveTreeScope({ type: "kind", section: "clusterOverview", kind: "Node" }),
    ).toMatchObject({
      section: "clusterOverview",
      namespace: null,
      kinds: ["Node"],
      clusterScoped: true,
      argoMode: false,
    });

    expect(resolveTreeScope({ type: "section", section: "argo" })).toMatchObject({
      section: "argo",
      argoMode: true,
    });
  });

  test("resolves discovered resource kind nodes", () => {
    const resourceKind: DiscoveredResourceKind = {
      group: "example.com",
      version: "v1",
      apiVersion: "example.com/v1",
      kind: "Widget",
      plural: "widgets",
      namespaced: true,
    };

    expect(
      resolveTreeScope({
        type: "kind",
        section: "discovered",
        kind: "example.com/v1/widgets/Widget",
        resourceKind,
      }),
    ).toMatchObject({
      section: "discovered",
      namespace: null,
      kinds: [resourceKind],
      clusterScoped: false,
      argoMode: false,
    });
  });

  test("explains empty states from scope", () => {
    expect(emptyStateMessage(resolveTreeScope(null), false)).toBe("Select a cluster context first");
    expect(emptyStateMessage(resolveTreeScope({ type: "section", section: "namespaces" } as TreeNodeId), true)).toBe("Select a namespace");
    expect(emptyStateMessage(resolveTreeScope({ type: "section", section: "argo" } as TreeNodeId), true)).toBe("Select an Argo CD resource type");
    expect(emptyStateMessage(resolveTreeScope({ type: "section", section: "discovered" } as TreeNodeId), true)).toBe("Select a discovered resource kind");
  });
});

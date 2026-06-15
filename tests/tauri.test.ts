import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import type {
  DiscoveredResourceKind,
  ResourceEventSummary,
  ResourceSummary,
} from "../src/lib/types";
import {
	buildIncidentSignals,
	conditionStatusTone,
	getContainerStatusRows,
	getConditionRows,
	incidentSignalCardClassName,
	resourceReadyLabel,
	resourceReadyTone,
	shouldFetchResourceEvents,
	shouldFetchResourceDetails,
} from "../src/features/resource-detail/helpers";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResources,
	filterResourceSearchIndex,
	filterResourcesByHealth,
	formatResourceGroupLabel,
	resourceIdentityKey,
	resourceGroupCollapseKey,
	resourceSelectionKey,
	resourceTypeGroupCollapseKey,
	formatResourceTypeGroupLabel,
	sortedRows,
	tableTooltipText,
	topologyWatchKeys,
	uniqueArgoApps,
	watchKeysFromFetchKeys,
} from "../src/features/resources/helpers";
import { groupHelmReleasesByNamespace } from "../src/features/helm/helpers";

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

describe("resource detail status tones", () => {
	const completedPod: ResourceSummary = {
		cluster: "kind-prod",
		kind: "Pod",
		name: "job-pod",
		namespace: "jobs-lab",
		age: "6m",
		health: "healthy",
		status: "Succeeded",
		ready: "False",
	};

	test("shows completed pods as not-dangerous when readiness is false", () => {
		expect(resourceReadyLabel(completedPod)).toBe("Completed");
		expect(resourceReadyTone(completedPod)).toBe("success");
	});

	test("renders complete phase chips as success", () => {
		const source = readFileSync(
			"src/features/resource-detail/DetailStatusField.tsx",
			"utf8",
		);

		expect(source).toContain('"Complete"');
		expect(source).toContain('"Completed"');
	});

	test("uses neutral condition chips for expected completed pod false conditions", () => {
		expect(
			conditionStatusTone(
				{ type: "Ready", status: "False", reason: "PodCompleted" },
				completedPod,
			),
		).toBe("neutral");
		expect(
			conditionStatusTone(
				{ type: "ContainersReady", status: "False", reason: "PodCompleted" },
				completedPod,
			),
		).toBe("neutral");
		expect(
			conditionStatusTone(
				{ type: "PodReadyToStartContainers", status: "False" },
				completedPod,
			),
		).toBe("neutral");
	});

	test("keeps active not-ready pods red", () => {
		const runningPod = {
			...completedPod,
			status: "Running",
			ready: "False",
			health: "degraded" as const,
		};

		expect(resourceReadyLabel(runningPod)).toBe("Not ready");
		expect(resourceReadyTone(runningPod)).toBe("error");
		expect(
			conditionStatusTone(
				{ type: "Ready", status: "False", reason: "ContainersNotReady" },
				runningPod,
			),
		).toBe("error");
	});

	test("marks true failure conditions as warning", () => {
		const failedJob = {
			...completedPod,
			kind: "Job",
			status: "Failed",
			ready: undefined,
			health: "degraded" as const,
		};

		expect(
			conditionStatusTone(
				{
					type: "FailureTarget",
					status: "True",
					reason: "BackoffLimitExceeded",
				},
				failedJob,
			),
		).toBe("warning");
		expect(
			conditionStatusTone(
				{ type: "Failed", status: "True", reason: "BackoffLimitExceeded" },
				failedJob,
			),
		).toBe("warning");
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
    health: "unknown",
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
      { ...baseResource, name: "api-0", status: "Running", ready: "True", health: "healthy", restarts: 0 },
      { ...baseResource, name: "worker-0", status: "Pending", health: "attention", restarts: 0 },
      { ...baseResource, name: "job-0", status: "Failed", ready: "False", health: "degraded", restarts: 1 },
    ]);

    expect(summary).toEqual({
      total: 3,
      healthy: 1,
      attention: 1,
      degraded: 1,
      restarted: 0,
      untracked: 0,
    });
  });

  test("filters resources by transient health filter", () => {
    const resources: ResourceSummary[] = [
      { ...baseResource, name: "api-0", status: "Running", ready: "True", health: "healthy", restarts: 0 },
      { ...baseResource, name: "worker-0", status: "Pending", health: "attention", restarts: 0 },
      { ...baseResource, name: "job-0", status: "Failed", ready: "False", health: "degraded", restarts: 1 },
      { ...baseResource, name: "cache-0", status: "Running", ready: "True", health: "restarted", restarts: 2 },
    ];

    expect(filterResourcesByHealth(resources, "all")).toEqual(resources);
    expect(filterResourcesByHealth(resources, "healthy").map((r) => r.name)).toEqual(["api-0"]);
    expect(filterResourcesByHealth(resources, "attention").map((r) => r.name)).toEqual(["worker-0"]);
    expect(filterResourcesByHealth(resources, "degraded").map((r) => r.name)).toEqual(["job-0"]);
    expect(filterResourcesByHealth(resources, "restarted").map((r) => r.name)).toEqual(["cache-0"]);
  });

  test("builds fetch keys for namespaced and cluster-scoped kinds", () => {
    expect(buildFetchKeys(["default", "payments"], ["Pod", "Node"])).toEqual([
      { kind: "Pod", namespace: "default" },
      { kind: "Pod", namespace: "payments" },
      { kind: "Node", namespace: undefined },
    ]);
  });

  test("builds selection identities that survive missing apiVersion", () => {
    const tableResource: ResourceSummary = {
      ...baseResource,
      kind: "DaemonSet",
      name: "cilium",
      namespace: "kube-system",
    };
    const topologyResource: ResourceSummary = {
      ...tableResource,
      apiVersion: "apps/v1",
    };

    expect(resourceSelectionKey(tableResource)).not.toBe(
      resourceSelectionKey(topologyResource),
    );
    expect(resourceIdentityKey(tableResource)).toBe(
      resourceIdentityKey(topologyResource),
    );
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

  test("keeps large namespace watch sets namespace-scoped for limited RBAC", () => {
    const namespaces = [
      "default",
      "payments",
      "jobs",
      "kube-system",
      "monitoring",
      "platform",
      "staging",
      "prod",
    ];

    expect(watchKeysFromFetchKeys([
      ...namespaces.map((namespace) => ({ kind: "Pod" as const, namespace })),
      { kind: "Service", namespace: "default" },
    ])).toEqual([
      ...namespaces.map((namespace) => ({
        resourceKind: { kind: "Pod" },
        namespace,
      })),
      { resourceKind: { kind: "Service" }, namespace: "default" },
    ]);
  });

  test("watches EndpointSlices for topology invalidation", () => {
    expect(topologyWatchKeys(["default"])).toContainEqual({
      resourceKind: { kind: "EndpointSlice" },
      namespace: "default",
    });
  });

  test("keeps large topology watch scopes namespace-scoped for limited RBAC", () => {
    const namespaces = [
      "default",
      "payments",
      "jobs",
      "kube-system",
      "monitoring",
      "platform",
      "staging",
      "prod",
    ];

    expect(topologyWatchKeys(namespaces)).toContainEqual({
      resourceKind: { kind: "EndpointSlice" },
      namespace: "prod",
    });
    expect(topologyWatchKeys(namespaces).length).toBe(104);
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

  test("reuses a resource search index across repeated searches", () => {
    const resources: ResourceSummary[] = [
      { ...baseResource, name: "api-0", ownerRef: "api", argoApp: "payments" },
      { ...baseResource, name: "worker-0", helmRelease: "jobs", argoApp: "batch" },
    ];
    const index = buildResourceSearchIndex(resources);

    expect(filterResourceSearchIndex(index, "jobs", "").map((r) => r.name)).toEqual(["worker-0"]);
    expect(filterResourceSearchIndex(index, "api", "payments").map((r) => r.name)).toEqual(["api-0"]);
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

  test("groups Helm releases by namespace", () => {
    const releases = [
      {
        cluster: "kind-dev",
        name: "worker",
        namespace: "jobs",
        age: "1m",
        storageKind: "Secret",
        storageName: "sh.helm.release.v1.worker.v1",
      },
      {
        cluster: "kind-dev",
        name: "api",
        namespace: "payments",
        age: "1m",
        storageKind: "Secret",
        storageName: "sh.helm.release.v1.api.v2",
      },
    ];
    expect(groupHelmReleasesByNamespace(releases).map((group) => group.namespace)).toEqual(["jobs", "payments"]);
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
    expect(describeResourceScope(["payments"], ["Pod", "Service"], "argocd")).toEqual([
      { kind: "namespaces", label: "Namespace", value: "payments" },
      { kind: "kinds", label: "Kinds", value: "Pod, Service" },
      { kind: "gitOpsOwner", label: "GitOps", value: "argocd" },
    ]);
    expect(describeResourceScope([], ["Pod"], "")).toEqual([
      { kind: "namespaces", label: "Namespaces", value: "All namespaces" },
      { kind: "kinds", label: "Kind", value: "Pod" },
    ]);
  });

  test("uses readable group labels for Argo-managed and unmanaged resources", () => {
    expect(formatResourceGroupLabel({ ...baseResource, argoApp: "argocd" })).toBe("Owned by Argo CD: argocd");
    expect(formatResourceGroupLabel({ ...baseResource, ownerRef: "api" })).toBe("Unmanaged resources");
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

    expect(resourceGroupCollapseKey(resource)).toBe("group:Owned by Argo CD: argocd");
    expect(resourceTypeGroupCollapseKey(resource)).toBe("group:Owned by Argo CD: argocd::type:ConfigMaps");
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

});

describe("sidebar source safeguards", () => {
	test("uses query state for namespace loading", () => {
		const source = readFileSync("src/components/NamespaceList.tsx", "utf8");

		expect(source).toContain('useQuery({');
		expect(source).toContain("queryKeys.namespaces");
		expect(source).not.toContain("requestSeq");
		expect(source).not.toContain("useEffect");
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
			"src/features/resource-detail/ResourceDiagnosticLists.tsx",
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
			"details?.summary ? { ...resource, ...details.summary } : resource",
		);
		expect(source).toContain(
			'currentResource.kind === "Pod" && (details || podDetailsLoading)',
		);
		expect(source).toContain("details || podDetailsLoading");
		expect(source).toContain("? containerRows");
		expect(source).toContain(": undefined");
		expect(source).not.toContain(
			'resource.kind === "Pod" || details ? containerRows : undefined',
		);
	});

	test("renders signal timestamps through the shared timestamp formatter", () => {
		const summarySource = readFileSync(
			"src/features/resource-detail/IncidentSummary.tsx",
			"utf8",
		);
		const valueSource = readFileSync(
			"src/features/resource-detail/IncidentSignalValue.tsx",
			"utf8",
		);

		expect(summarySource).toContain("<IncidentSignalValue signal={signal} />");
		expect(valueSource).toContain("ExactTimestampText");
		expect(valueSource).toContain("valueParts");
		expect(valueSource).toContain('precision="millisecond"');
		expect(valueSource).not.toContain("{signal.value}");
	});

	test("renders detail timeline timestamps with millisecond precision", () => {
		const source = readFileSync(
			"src/features/resource-detail/IncidentTimeline.tsx",
			"utf8",
		);

		expect(source).toContain(
			'<ExactTimestampText value={item.timestamp} precision="millisecond" />',
		);
	});

	test("renders metadata creation timestamps through the shared timestamp formatter", () => {
		const source = readFileSync(
			"src/features/resource-detail/ResourceDiagnostics.tsx",
			"utf8",
		);

		expect(source).toContain('name === "Created"');
		expect(source).toContain(
			'<ExactTimestampText value={value} precision="millisecond" />',
		);
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

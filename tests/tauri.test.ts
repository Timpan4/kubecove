import { describe, test, expect } from "bun:test";
import { createMockTauriClient, listKubeContexts, listNamespaces, getResourceYaml, isAppError } from "../src/lib/tauri";
import type {
  ClusterContext,
  DiscoveredResourceKind,
  NamespaceSummary,
  ResourceEventSummary,
  ResourceSummary,
} from "../src/lib/types";
import {
	buildIncidentSignals,
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
} from "../src/features/resources/helpers";
import {
	pageAppGroupCounts,
	pageTypeGroupCounts,
} from "../src/features/resources/grouping";
import {
	emptyStateMessage,
	resolveTreeScope,
	type TreeNodeId,
} from "../src/lib/tree-nav";

describe("createMockTauriClient", () => {
  test("returns mock response for known command", async () => {
    const mockContexts: ClusterContext[] = [{ name: "minikube" }, { name: "docker-desktop" }];
    const client = createMockTauriClient({
      list_kube_contexts: mockContexts,
    });

    const result = await listKubeContexts(client);
    expect(result).toEqual(mockContexts);
  });

  test("throws for unknown command", async () => {
    const client = createMockTauriClient({});

    let threw = false;
    try {
      await listKubeContexts(client);
    } catch {
      threw = true;
    }
    expect(threw).toBe(true);
  });
});

describe("listKubeContexts", () => {
  test("returns cluster contexts from client", async () => {
    const mockContexts: ClusterContext[] = [{ name: "minikube" }];
    const mockClient = createMockTauriClient({ list_kube_contexts: mockContexts });

    const result = await listKubeContexts(mockClient);
    expect(result).toEqual(mockContexts);
    expect(result[0].name).toBe("minikube");
  });
});

describe("listNamespaces", () => {
  test("passes cluster_context to client", async () => {
    const mockNamespaces: NamespaceSummary[] = [
      { name: "default", age: "2024-01-01T00:00:00Z" },
    ];
    const client = createMockTauriClient({ list_namespaces: mockNamespaces });

    const result = await listNamespaces(client, "minikube");
    expect(result).toEqual(mockNamespaces);
  });
});

describe("isAppError", () => {
  test("returns true for valid AppError", () => {
    const err = { message: "test", kind: "cluster" };
    expect(isAppError(err)).toBe(true);
  });

  test("returns false for non-AppError object", () => {
    expect(isAppError({ message: "test" })).toBe(false);
    expect(isAppError({ kind: "cluster" })).toBe(false);
    expect(isAppError(null)).toBe(false);
  });
});

describe("getResourceYaml", () => {
  test("returns raw YAML string from client", async () => {
    const mockYaml = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: test-pod";
    const client = createMockTauriClient({ get_resource_yaml: mockYaml });

    const result = await getResourceYaml(client, "minikube", "Pod", "test-pod", "default");
    expect(result).toBe(mockYaml);
    expect(typeof result).toBe("string");
  });
});

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
          },
        ],
      }),
    ).toEqual([
      {
        type: "Ready",
        status: "False",
        reason: "ContainersNotReady",
        message: "containers with unready status",
      },
    ]);
  });

  test("returns an empty list when status conditions are absent", () => {
    expect(getConditionRows({ phase: "Running" })).toEqual([]);
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

  test("counts page groups for grouped resource tables", () => {
    const rows: ResourceSummary[] = [
      { ...baseResource, name: "api", kind: "Pod", argoApp: "payments" },
      { ...baseResource, name: "svc", kind: "Service", argoApp: "payments" },
      { ...baseResource, name: "deploy-pod", kind: "Pod", ownerRef: "api" },
      { ...baseResource, name: "cm", kind: "ConfigMap" },
    ];

    expect(pageAppGroupCounts(rows, true)).toEqual(
      new Map([
        ["Managed by Argo app: payments", 2],
        ["Owned by: api", 1],
        ["Unmanaged resources", 1],
      ]),
    );
    expect(pageTypeGroupCounts(rows, true)).toEqual(
      new Map([
        ["Managed by Argo app: payments::Pods", 1],
        ["Managed by Argo app: payments::Services", 1],
        ["Owned by: api::Pods", 1],
        ["Unmanaged resources::ConfigMaps", 1],
      ]),
    );
    expect(pageAppGroupCounts(rows, false).size).toBe(0);
    expect(pageTypeGroupCounts(rows, false).size).toBe(0);
  });

  test("normalizes tooltip values for table display", () => {
    expect(tableTooltipText("argocd-server-7886b899c8-l5lqd")).toBe("argocd-server-7886b899c8-l5lqd");
    expect(tableTooltipText(null)).toBe("—");
    expect(tableTooltipText("")).toBe("—");
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
        value: "Ready=False · ContainersNotReady",
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

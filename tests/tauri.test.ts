import { describe, test, expect } from "bun:test";
import { createMockTauriClient, listKubeContexts, listNamespaces, getResourceYaml, isAppError } from "../src/lib/tauri";
import type { ClusterContext, NamespaceSummary, ResourceSummary } from "../src/lib/types";
import {
	getConditionRows,
	shouldFetchResourceDetails,
} from "../src/features/resource-detail/ResourceDetailPanel";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	describeResourceScope,
	filterResources,
	formatResourceGroupLabel,
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
	formatResourceTypeGroupLabel,
	sortedRows,
	tableTooltipText,
	uniqueArgoApps,
} from "../src/features/resources/helpers";
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

  test("builds fetch keys for namespaced and cluster-scoped kinds", () => {
    expect(buildFetchKeys(["default", "payments"], ["Pod", "Node"])).toEqual([
      { kind: "Pod", namespace: "default" },
      { kind: "Pod", namespace: "payments" },
      { kind: "Node", namespace: undefined },
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

    expect(resourceGroupCollapseKey(resource)).toBe("app:Managed by Argo app: argocd");
    expect(resourceTypeGroupCollapseKey(resource)).toBe("app:Managed by Argo app: argocd::type:ConfigMaps");
  });

  test("normalizes tooltip values for table display", () => {
    expect(tableTooltipText("argocd-server-7886b899c8-l5lqd")).toBe("argocd-server-7886b899c8-l5lqd");
    expect(tableTooltipText(null)).toBe("—");
    expect(tableTooltipText("")).toBe("—");
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

  test("explains empty states from scope", () => {
    expect(emptyStateMessage(resolveTreeScope(null), false)).toBe("Select a cluster context first");
    expect(emptyStateMessage(resolveTreeScope({ type: "section", section: "namespaces" } as TreeNodeId), true)).toBe("Select a namespace");
    expect(emptyStateMessage(resolveTreeScope({ type: "section", section: "argo" } as TreeNodeId), true)).toBe("Select an Argo CD resource type");
  });
});

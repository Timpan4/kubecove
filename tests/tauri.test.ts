import { describe, test, expect } from "bun:test";
import { createMockTauriClient, listKubeContexts, listNamespaces, getResourceYaml, isAppError } from "../src/lib/tauri";
import type { ClusterContext, NamespaceSummary, ResourceSummary } from "../src/lib/types";
import {
  getConditionRows,
  shouldFetchResourceDetails,
} from "../src/features/resource-detail/ResourceDetailPanel";
import {
  buildResourceHealthSummary,
  describeResourceScope,
  formatResourceGroupLabel,
  resourceGroupCollapseKey,
  resourceTypeGroupCollapseKey,
  formatResourceTypeGroupLabel,
  tableTooltipText,
} from "../src/components/ResourceList";

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

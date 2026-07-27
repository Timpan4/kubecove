import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import type { DiscoveredResourceKind, ResourceSummary } from "../src/lib/types";
import {
	buildFetchKeys,
	buildResourceHealthSummary,
	buildResourceSearchIndex,
	describeResourceScope,
	filterResources,
	filterResourceSearchIndex,
	filterResourcesByHealth,
	formatResourceGroupLabel,
	formatResourceTypeGroupLabel,
	resourceGroupCollapseKey,
	resourceIdentityKey,
	resourceSelectionKey,
	resourceTypeGroupCollapseKey,
	sortedRows,
	tableTooltipText,
	topologyWatchKeys,
	uniqueArgoApps,
	watchKeysFromFetchKeys,
} from "../src/features/resources/helpers";
import { groupHelmReleasesByNamespace } from "../src/features/helm/helpers";

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

  test("pluralizes type subgroup labels", () => {
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Pod" })).toBe("Pods");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "ConfigMap" })).toBe("ConfigMaps");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Gateway" })).toBe("Gateways");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Policy" })).toBe("Policies");
    expect(formatResourceTypeGroupLabel({ ...baseResource, kind: "Ingress" })).toBe("Ingresses");
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
    const source = readFileSync("src/features/resources/ResourceBrowser.svelte", "utf8");

    expect(source).toContain('class="flex justify-center"');
    expect(source).toContain("row.restarts");
  });

  test("uses pointer cursors for enabled shared interactive controls", () => {
    const controlFiles = [
      "src/components/ui/svelte/classes.ts",
      "src/components/ui/svelte/TabsTrigger.svelte",
      "src/components/ui/svelte/SelectTrigger.svelte",
      "src/components/ui/svelte/Checkbox.svelte",
    ];

    for (const file of controlFiles) {
      expect(readFileSync(file, "utf8")).toContain("cursor-pointer");
    }
  });

});

/**
 * IDE-style sidebar tree for Kubernetes resource navigation.
 *
 * Tree structure:
 * - Workspace Overview [section, saved workspace summary]
 * - Cluster Overview [section, cluster-scoped kinds]
 * - Namespaces [section, dynamic namespace nodes]
 *   - <namespace> [namespace node, expands to group children]
 *     - Workloads [group node]
 *       - Pod, Deployment, StatefulSet, DaemonSet, Job, CronJob [kind nodes]
 *     - Network
 *       - Service, Ingress
 *     - Config
 *       - ConfigMap, Secret
 *     - Storage
 *       - PersistentVolumeClaim, PersistentVolume, StorageClass
 * - Workloads [section — top-level shortcut, same kinds as namespace children]
 * - Network [section shortcut]
 * - Config [section shortcut]
 * - Storage [section shortcut]
 * - GitOps [section]
 *   - Argo CD and Flux provider child nodes
 * - Helm [section]
 *   - Releases [read-only Helm v3 release inventory]
 * - Incidents [section]
 * - Port Forwards [section]
 * - RBAC [section]
 *   - Namespace Access, Roles, Cluster Roles, Bindings, Service Accounts
 */

import { useCallback, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  createTauriClient,
  detectArgoCD,
  detectFlux,
  listNamespaces,
  listResourceKinds,
} from "../lib/tauri";
import { useSettingsState } from "@/lib/settings";
import {
  ARGO_NAV_KINDS,
  ARGO_PROVIDER_GROUP_ID,
  FLUX_FAMILIES,
  FLUX_PROVIDER_GROUP_ID,
} from "@/features/gitops/gitops-nav";
import { queryKeys } from "@/lib/queryKeys";
import {
  type TreeNodeId,
  type TreeNode,
  SECTIONS,
  discoveredResourceKindKey,
  nodeIdToString,
} from "../lib/tree-nav";
import { cn } from "@/lib/utils";
import { ChevronRight } from "lucide-react";
import {
  getResourceGroupVisual,
  getResourceKindVisual,
} from "@/lib/resource-visuals";
import {
  buildNamespaceTreeNode,
  buildShallowNamespaceTreeNode,
} from "./sidebar-tree-helpers";

function buildArgoProviderNode(disabled: boolean): TreeNode {
  return {
    id: { type: "group", section: "argo", group: ARGO_PROVIDER_GROUP_ID },
    label: "Argo CD",
    disabled,
    description: disabled ? "Argo CD CRDs were not detected in this cluster." : undefined,
    children: disabled
      ? []
      : ARGO_NAV_KINDS.map((kind) => ({
          id: {
            type: "kind",
            section: "argo",
            namespace: undefined,
            group: ARGO_PROVIDER_GROUP_ID,
            kind: kind.label,
          },
          label: kind.label,
        })),
  };
}

function buildFluxProviderNode(disabled: boolean): TreeNode {
  return {
    id: { type: "group", section: "argo", group: FLUX_PROVIDER_GROUP_ID },
    label: "Flux",
    disabled,
    description: disabled ? "Flux CRDs were not detected in this cluster." : undefined,
    children: disabled
      ? []
      : FLUX_FAMILIES.map((family) => ({
          id: {
            type: "group",
            section: "argo",
            group: family.groupId,
          },
          label: family.label,
          children: family.kinds.map((kind) => ({
            id: {
              type: "kind",
              section: "argo",
              namespace: undefined,
              group: family.groupId,
              kind: kind.label,
            },
            label: kind.label,
          })),
        })),
  };
}

interface SidebarTreeProps {
  clusterContext: string;
  selectedNode: TreeNodeId | null;
  expandedSections: string[];
  onNodeSelect: (id: TreeNodeId) => void;
  onSectionToggle: (section: string) => void;
}

interface TreeNodeProps {
  node: TreeNode;
  depth: number;
  selectedNode: TreeNodeId | null;
  expandedSections: ReadonlySet<string>;
  onNodeSelect: (id: TreeNodeId) => void;
  onSectionToggle: (section: string) => void;
  hasChildren: boolean;
  getLazyChildren?: (node: TreeNode) => TreeNode[] | undefined;
}

function ChevronIcon({ expanded }: { expanded: boolean }) {
  return (
    <ChevronRight
      className={cn("size-3 shrink-0 transition-transform", expanded && "rotate-90")}
    />
  );
}

function getNodeVisual(node: TreeNode) {
  if (node.id.type === "kind" && node.id.kind) {
    return getResourceKindVisual(node.id.kind);
  }
  if (node.id.type === "namespace") {
    return getResourceGroupVisual("Namespaces");
  }
  return getResourceGroupVisual(node.label);
}

function TreeNodeComponent({
  node,
  depth,
  selectedNode,
  expandedSections,
  onNodeSelect,
  onSectionToggle,
  hasChildren,
  getLazyChildren,
}: TreeNodeProps) {
  const idStr = nodeIdToString(node.id);
  const isSelected = selectedNode !== null && nodeIdToString(selectedNode) === idStr;
  const isExpanded = expandedSections.has(idStr);
  const isDisabled = node.disabled === true;
  const children = isExpanded ? getLazyChildren?.(node) ?? node.children : node.children;
  const visual = getNodeVisual(node);
  const NodeIcon = visual.icon;
  const depthPaddingClass =
    depth === 0
      ? "pl-2"
      : depth === 1
        ? "pl-6"
        : depth === 2
          ? "pl-10"
          : depth === 3
            ? "pl-14"
            : "pl-[72px]";

  const selectTreeNode = () => {
    if (isDisabled) {
      return;
    }
    onNodeSelect(node.id);
    if (hasChildren) {
      onSectionToggle(idStr);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectTreeNode();
      return;
    }

    if (!hasChildren) {
      return;
    }

    if (e.key === "ArrowRight" && !isExpanded) {
      e.preventDefault();
      onSectionToggle(idStr);
    } else if (e.key === "ArrowLeft" && isExpanded) {
      e.preventDefault();
      onSectionToggle(idStr);
    }
  };

  const handleChevronClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isDisabled) {
      onNodeSelect(node.id);
    }
    onSectionToggle(idStr);
  };

  const handleChevronKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      if (!isDisabled) {
        onNodeSelect(node.id);
      }
      onSectionToggle(idStr);
    }
  };

  return (
    <li>
      <div
        className={cn(
          "relative flex h-[26px] cursor-pointer select-none items-center gap-1 rounded-none text-[0.8125rem] text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
          isDisabled &&
            "cursor-default text-muted-foreground/70 hover:bg-transparent hover:text-muted-foreground/70",
          isSelected &&
            "bg-sidebar-accent text-sidebar-accent-foreground before:absolute before:bottom-0 before:left-0 before:top-0 before:w-0.5 before:rounded-r-sm before:bg-sidebar-primary",
          depth === 0 &&
            "text-[0.6875rem] font-bold uppercase tracking-wide text-muted-foreground hover:bg-transparent hover:text-foreground/70",
          depthPaddingClass,
        )}
        data-depth={depth}
        onClick={selectTreeNode}
        onKeyDown={handleKeyDown}
        role="treeitem"
        tabIndex={isDisabled ? -1 : 0}
        aria-selected={isSelected}
        aria-expanded={hasChildren ? isExpanded : undefined}
        aria-disabled={isDisabled || undefined}
        title={node.description}
      >
        <button
          type="button"
          className={cn(
            "flex size-[18px] shrink-0 cursor-pointer items-center justify-center border-0 bg-transparent p-0 text-muted-foreground transition-colors hover:text-foreground",
            !hasChildren && "invisible",
          )}
          onClick={handleChevronClick}
          onKeyDown={handleChevronKeyDown}
          aria-label={`${isExpanded ? "Collapse" : "Expand"} ${node.label}`}
          disabled={!hasChildren}
        >
          <ChevronIcon expanded={isExpanded} />
        </button>
        <NodeIcon
          className={cn(
            "size-3.5 shrink-0",
            depth === 0 ? "size-3" : "size-3.5",
            visual.className,
          )}
        />
        <span className="min-w-0 flex-1 truncate leading-none">{node.label}</span>
      </div>
      {hasChildren && isExpanded && children && (
        <ul className="m-0 list-none p-0" role="group">
          {children.map((child) => (
            <TreeNodeComponent
              key={nodeIdToString(child.id)}
              node={child}
              depth={depth + 1}
              selectedNode={selectedNode}
              expandedSections={expandedSections}
              onNodeSelect={onNodeSelect}
              onSectionToggle={onSectionToggle}
              hasChildren={
                child.id.type === "namespace" ||
                (!!child.children && child.children.length > 0)
              }
              getLazyChildren={getLazyChildren}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export function SidebarTree({
  clusterContext,
  selectedNode,
  expandedSections,
  onNodeSelect,
  onSectionToggle,
}: SidebarTreeProps) {
  const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
  const showUnavailableGitOpsProviders = useSettingsState(
    (state) => state.showUnavailableGitOpsProviders,
  );
  const client = useMemo(() => createTauriClient(), []);
  const {
    data: namespaces = [],
    isPending: loading,
    isError,
    error,
  } = useQuery({
    queryKey: queryKeys.namespaces(clusterContext, kubeconfigEnvVar),
    queryFn: () => listNamespaces(client, clusterContext, kubeconfigEnvVar),
    enabled: Boolean(clusterContext),
  });
  const {
    data: resourceKinds = [],
    isPending: resourceKindsLoading,
    isError: resourceKindsIsError,
    error: resourceKindsErr,
  } = useQuery({
    queryKey: queryKeys.resourceKinds(clusterContext, kubeconfigEnvVar),
    queryFn: () => listResourceKinds(client, clusterContext, kubeconfigEnvVar),
    enabled: Boolean(clusterContext),
  });
  const namespaceErrorMessage =
    error instanceof Error ? error.message : "Failed to load namespaces";
  const resourceKindsError =
    resourceKindsErr instanceof Error
      ? resourceKindsErr.message
      : "Failed to load discovered resources";
  const {
    data: argoDetection,
    isPending: argoDetectionIsPending,
    isSuccess: argoDetectionIsSuccess,
  } = useQuery({
    queryKey: queryKeys.argoDetect(clusterContext, kubeconfigEnvVar),
    queryFn: () => detectArgoCD(client, clusterContext, kubeconfigEnvVar),
    enabled: !!clusterContext,
    staleTime: 60_000,
  });
  const {
    data: fluxDetection,
    isPending: fluxDetectionIsPending,
    isSuccess: fluxDetectionIsSuccess,
  } = useQuery({
    queryKey: queryKeys.fluxDetect(clusterContext, kubeconfigEnvVar),
    queryFn: () => detectFlux(client, clusterContext, kubeconfigEnvVar),
    enabled: !!clusterContext,
    staleTime: 60_000,
  });

  // Build the static tree (no namespace children yet — those are built dynamically)
  const staticTree = useMemo(() => {
    const workspaceOverviewNode: TreeNode = {
      id: { type: "section", section: "workspaceOverview" },
      label: SECTIONS.workspaceOverview.label,
    };

    const clusterOverviewNode: TreeNode = {
      id: { type: "section", section: "clusterOverview" },
      label: SECTIONS.clusterOverview.label,
      children: (["Node", "StorageClass", "PersistentVolume"] as const).map((kind) => ({
        id: { type: "kind", section: "clusterOverview", namespace: undefined, group: undefined, kind },
        label: kind,
      })),
    };

    const curatedSections: Array<{ id: string; label: string; children: readonly string[] }> = [
      { id: "workloads", label: SECTIONS.workloads.label, children: SECTIONS.workloads.children },
      { id: "network", label: SECTIONS.network.label, children: SECTIONS.network.children },
      { id: "config", label: SECTIONS.config.label, children: SECTIONS.config.children },
      { id: "storage", label: SECTIONS.storage.label, children: SECTIONS.storage.children },
    ];

    const curatedSectionNodes = curatedSections.map((sec): TreeNode => ({
      id: { type: "section", section: sec.id },
      label: sec.label,
      children: sec.children.map((child) => ({
        id: { type: "kind", section: sec.id, namespace: undefined, group: undefined, kind: child } as TreeNodeId,
        label: child,
      })),
    }));

    const helmNode: TreeNode = {
      id: { type: "section", section: "helm" },
      label: SECTIONS.helm.label,
      children: SECTIONS.helm.children.map((child) => ({
        id: { type: "kind", section: "helm", namespace: undefined, group: undefined, kind: child },
        label: child,
      })),
    };

    const portForwardsNode: TreeNode = {
      id: { type: "section", section: "portForwards" },
      label: SECTIONS.portForwards.label,
    };

    const incidentsNode: TreeNode = {
      id: { type: "section", section: "incidents" },
      label: SECTIONS.incidents.label,
    };

    const rbacNode: TreeNode = {
      id: { type: "section", section: "rbac" },
      label: SECTIONS.rbac.label,
      children: SECTIONS.rbac.children.map((child) => ({
        id: { type: "kind", section: "rbac", namespace: undefined, group: undefined, kind: child },
        label: child,
      })),
    };

    return { workspaceOverviewNode, clusterOverviewNode, curatedSectionNodes, helmNode, incidentsNode, portForwardsNode, rbacNode };
  }, []);

  const gitOpsNode = useMemo<TreeNode>(() => {
    const children: TreeNode[] = [];
    const argoDetected = argoDetection === true;
    const fluxDetected = fluxDetection?.detected === true;
    const argoKnownUnavailable =
      argoDetectionIsSuccess && argoDetection === false;
    const fluxKnownUnavailable =
      fluxDetectionIsSuccess && fluxDetection?.detected === false;

    if (argoDetected || (showUnavailableGitOpsProviders && argoKnownUnavailable)) {
      children.push(buildArgoProviderNode(!argoDetected));
    }
    if (fluxDetected || (showUnavailableGitOpsProviders && fluxKnownUnavailable)) {
      children.push(buildFluxProviderNode(!fluxDetected));
    }
    if (
      children.length === 0 &&
      (argoDetectionIsPending || fluxDetectionIsPending)
    ) {
      children.push({
        id: { type: "group", section: "argo", group: "gitops:detecting" },
        label: "Detecting providers...",
        disabled: true,
      });
    }
    return {
      id: { type: "section", section: "argo" },
      label: SECTIONS.argo.label,
      children,
    };
  }, [
    argoDetection,
    argoDetectionIsPending,
    argoDetectionIsSuccess,
    fluxDetection?.detected,
    fluxDetectionIsPending,
    fluxDetectionIsSuccess,
    showUnavailableGitOpsProviders,
  ]);

  const extraKinds = useMemo(() => {
    const curatedKindKeys = new Set<string>([
      "/Pod",
      "/Service",
      "/ConfigMap",
      "/Secret",
      "/PersistentVolumeClaim",
      "/Node",
      "/PersistentVolume",
      "apps/Deployment",
      "apps/StatefulSet",
      "apps/DaemonSet",
      "batch/Job",
      "batch/CronJob",
      "networking.k8s.io/Ingress",
      "storage.k8s.io/StorageClass",
      "argoproj.io/Application",
      "argoproj.io/ApplicationSet",
      "argoproj.io/AppProject",
    ]);
    return resourceKinds
      .filter((resourceKind) => !curatedKindKeys.has(`${resourceKind.group}/${resourceKind.kind}`))
      .sort((a, b) =>
        a.kind.localeCompare(b.kind) ||
        a.apiVersion.localeCompare(b.apiVersion) ||
        a.plural.localeCompare(b.plural),
      );
  }, [resourceKinds]);

  const discoveredTree = useMemo<TreeNode>(() => {
    let children: TreeNode[];

    if (resourceKindsLoading) {
      children = [
        {
          id: { type: "kind", section: "discovered", kind: "__loading" },
          label: "Loading discovered kinds...",
          disabled: true,
        },
      ];
    } else if (resourceKindsIsError) {
      children = [
        {
          id: { type: "kind", section: "discovered", kind: "__error" },
          label: "Discovery failed",
          description: resourceKindsError,
          disabled: true,
        },
      ];
    } else {
      children =
        extraKinds.length > 0
          ? extraKinds.map((resourceKind) => ({
              id: {
                type: "kind",
                section: "discovered",
                kind: discoveredResourceKindKey(resourceKind),
                resourceKind,
              } as TreeNodeId,
              label: resourceKind.kind,
              description: `${resourceKind.apiVersion} / ${resourceKind.plural} / ${resourceKind.namespaced ? "namespaced" : "cluster-scoped"}`,
            }))
          : [
              {
                id: { type: "kind", section: "discovered", kind: "__empty" },
                label: "No extra kinds",
                disabled: true,
              },
            ];
    }

    return {
      id: { type: "section", section: "discovered" },
      label: SECTIONS.discovered.label,
      children,
    };
  }, [extraKinds, resourceKindsError, resourceKindsIsError, resourceKindsLoading]);

  const buildNamespaceChildren = useCallback(
    (namespace: string): TreeNode => {
      return buildNamespaceTreeNode(namespace, extraKinds);
    },
    [extraKinds]
  );

  const getLazyChildren = useCallback(
    (node: TreeNode): TreeNode[] | undefined => {
      if (node.id.type !== "namespace" || !node.id.namespace) {
        return node.children;
      }
      return buildNamespaceChildren(node.id.namespace).children;
    },
    [buildNamespaceChildren],
  );

  const expandedSectionSet = useMemo(
    () => new Set(expandedSections),
    [expandedSections],
  );

  // Full tree with shallow namespace children; deep namespace groups are lazy.
  const fullTree = useMemo<TreeNode[]>(() => {
    const namespaceNode: TreeNode = {
      id: { type: "section" as const, section: "namespaces" },
      label: SECTIONS.namespaces.label,
      children: namespaces.map((ns) => buildShallowNamespaceTreeNode(ns.name)),
    };
    return [
      staticTree.workspaceOverviewNode,
      staticTree.clusterOverviewNode,
      namespaceNode,
      ...staticTree.curatedSectionNodes,
      discoveredTree,
      gitOpsNode,
      staticTree.helmNode,
      staticTree.incidentsNode,
      staticTree.portForwardsNode,
      staticTree.rbacNode,
    ];
  }, [namespaces, staticTree, discoveredTree, gitOpsNode]);

  if (!clusterContext) {
    return (
      <div className="p-4 text-center text-xs text-muted-foreground">
        Select a cluster context to load the resource tree
      </div>
    );
  }

  if (loading) {
    return <div className="p-4 text-center text-xs text-muted-foreground">Loading namespaces…</div>;
  }

  if (isError) {
    return <div className="p-4 text-center text-xs text-destructive">{namespaceErrorMessage}</div>;
  }

  return (
    <nav className="flex-1 py-2" aria-label="Kubernetes resource tree">
      <ul className="m-0 list-none p-0" role="tree">
        {fullTree.map((node) => (
          <TreeNodeComponent
            key={nodeIdToString(node.id)}
            node={node}
            depth={0}
            selectedNode={selectedNode}
            expandedSections={expandedSectionSet}
            onNodeSelect={onNodeSelect}
            onSectionToggle={onSectionToggle}
            hasChildren={!!node.children && node.children.length > 0}
            getLazyChildren={getLazyChildren}
          />
        ))}
      </ul>
    </nav>
  );
}

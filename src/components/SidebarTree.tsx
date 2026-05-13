/**
 * IDE-style sidebar tree for Kubernetes resource navigation.
 *
 * Tree structure:
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
 * - Argo CD [section]
 *   - Applications, ApplicationSets, AppProjects [Argo child nodes]
 */

import { useState, useEffect, useCallback, useMemo } from "react";
import { createTauriClient, listNamespaces } from "../lib/tauri";
import type { NamespaceSummary } from "../lib/types";
import {
  type TreeNodeId,
  type TreeNode,
  SECTIONS,
  KIND_GROUPS,
  type KindGroupName,
  nodeIdToString,
} from "../lib/tree-nav";
import { cn } from "@/lib/utils";
import { ChevronRight } from "lucide-react";
import {
  getResourceGroupVisual,
  getResourceKindVisual,
} from "@/lib/resource-visuals";

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
  expandedSections: string[];
  onNodeSelect: (id: TreeNodeId) => void;
  onSectionToggle: (section: string) => void;
  hasChildren: boolean;
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
}: TreeNodeProps) {
  const idStr = nodeIdToString(node.id);
  const isSelected = selectedNode !== null && nodeIdToString(selectedNode) === idStr;
  const isExpanded = expandedSections.includes(idStr);
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

  const handleClick = () => {
    onNodeSelect(node.id);
  };

  const handleChevronClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onSectionToggle(idStr);
  };

  return (
    <li>
      <div
        className={cn(
          "relative flex h-[26px] cursor-pointer select-none items-center gap-1 rounded-none text-[0.8125rem] text-sidebar-foreground/80 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
          isSelected &&
            "bg-sidebar-accent text-sidebar-accent-foreground before:absolute before:bottom-0 before:left-0 before:top-0 before:w-0.5 before:rounded-r-sm before:bg-sidebar-primary",
          depth === 0 &&
            "text-[0.6875rem] font-bold uppercase tracking-wide text-muted-foreground hover:bg-transparent hover:text-foreground/70",
          depthPaddingClass,
        )}
        data-depth={depth}
        onClick={handleClick}
        role="treeitem"
        aria-expanded={hasChildren ? isExpanded : undefined}
      >
        <button
          className={cn(
            "flex size-[18px] shrink-0 cursor-pointer items-center justify-center border-0 bg-transparent p-0 text-muted-foreground transition-colors hover:text-foreground",
            !hasChildren && "invisible",
          )}
          onClick={handleChevronClick}
          tabIndex={-1}
          aria-hidden="true"
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
      {hasChildren && isExpanded && node.children && (
        <ul className="m-0 list-none p-0" role="group">
          {node.children.map((child) => (
            <TreeNodeComponent
              key={nodeIdToString(child.id)}
              node={child}
              depth={depth + 1}
              selectedNode={selectedNode}
              expandedSections={expandedSections}
              onNodeSelect={onNodeSelect}
              onSectionToggle={onSectionToggle}
              hasChildren={!!child.children && child.children.length > 0}
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
  const [namespaces, setNamespaces] = useState<NamespaceSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadNamespaces = useCallback(async () => {
    if (!clusterContext) {
      setNamespaces([]);
      return;
    }
    const client = createTauriClient();
    setLoading(true);
    setError(null);
    try {
      const ns = await listNamespaces(client, clusterContext);
      setNamespaces(ns);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load namespaces");
    } finally {
      setLoading(false);
    }
  }, [clusterContext]);

  useEffect(() => {
    loadNamespaces();
  }, [loadNamespaces]);

  // Build the static tree (no namespace children yet — those are built dynamically)
  const staticTree = useMemo<TreeNode[]>(() => {
    const nodes: TreeNode[] = [];

    // Cluster Overview
    nodes.push({
      id: { type: "section", section: "clusterOverview" },
      label: SECTIONS.clusterOverview.label,
      children: (["Node", "StorageClass", "PersistentVolume"] as const).map((kind) => ({
        id: { type: "kind", section: "clusterOverview", namespace: undefined, group: undefined, kind },
        label: kind,
      })),
    });

    // Static sections (Workloads, Network, Config, Storage, Argo CD)
    const staticSections: Array<{ id: string; label: string; children: readonly string[] }> = [
      { id: "workloads", label: SECTIONS.workloads.label, children: SECTIONS.workloads.children },
      { id: "network", label: SECTIONS.network.label, children: SECTIONS.network.children },
      { id: "config", label: SECTIONS.config.label, children: SECTIONS.config.children },
      { id: "storage", label: SECTIONS.storage.label, children: SECTIONS.storage.children },
      { id: "argo", label: SECTIONS.argo.label, children: SECTIONS.argo.children },
    ];

    for (const sec of staticSections) {
      const isArgo = sec.id === "argo";
      nodes.push({
        id: { type: "section", section: sec.id },
        label: sec.label,
        children: sec.children.map((child) => {
          if (isArgo) {
            return {
              id: { type: "kind", section: "argo", namespace: undefined, group: undefined, kind: child },
              label: child,
            };
          }
          // For static sections (Workloads/Network/Config/Storage), children are kinds directly,
          // not groups — there is no KIND_GROUPS entry for e.g. "Pod" as a group key
          return {
            id: { type: "kind", section: sec.id, namespace: undefined, group: undefined, kind: child } as TreeNodeId,
            label: child,
          };
        }),
      });
    }

    return nodes;
  }, []);

  // Build namespace subtree for a given namespace name
  const buildNamespaceSubtree = useCallback(
    (namespace: string): TreeNode => {
      const groups: TreeNode[] = (Object.keys(KIND_GROUPS) as KindGroupName[]).map((groupName) => {
        const kinds = KIND_GROUPS[groupName];
        return {
          id: { type: "group", section: "namespaces", namespace, group: groupName } as TreeNodeId,
          label: groupName,
          children: kinds.map((kind) => ({
            id: { type: "kind", section: "namespaces", namespace, group: groupName, kind } as TreeNodeId,
            label: kind,
          })),
        };
      });

      return {
        id: { type: "namespace", section: "namespaces", namespace },
        label: namespace,
        children: groups,
      };
    },
    []
  );

  // Full tree with namespace children
  const fullTree = useMemo<TreeNode[]>(() => {
    const namespaceNode: TreeNode = {
      id: { type: "section" as const, section: "namespaces" },
      label: SECTIONS.namespaces.label,
      children: namespaces.map((ns) => buildNamespaceSubtree(ns.name)),
    };
    return [staticTree[0], namespaceNode, ...staticTree.slice(1)];
  }, [namespaces, staticTree, buildNamespaceSubtree]);

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

  if (error) {
    return <div className="p-4 text-center text-xs text-destructive">{error}</div>;
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
            expandedSections={expandedSections}
            onNodeSelect={onNodeSelect}
            onSectionToggle={onSectionToggle}
            hasChildren={!!node.children && node.children.length > 0}
          />
        ))}
      </ul>
    </nav>
  );
}

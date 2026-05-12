/**
 * Tree navigation types, section definitions, kind groupings, and helpers
 * for the IDE-style sidebar tree.
 */

import { CLUSTER_SCOPED_KINDS, SUPPORTED_KINDS } from "./types";

// ─── Tree Node Types ───────────────────────────────────────────────────────────

export type TreeNodeType =
  | "section"          // top-level section (Cluster Overview, Namespaces, etc.)
  | "namespace"        // namespace leaf node
  | "group"            // kind group (Workloads, Network, Config, Storage)
  | "kind";            // resource kind leaf node

export interface TreeNodeId {
  type: TreeNodeType;
  section: string;           // section name
  namespace?: string;        // namespace name (for namespace/group/kind nodes)
  group?: string;            // group name (for kind nodes)
  kind?: string;             // kind name (for kind nodes)
}

/** A node in the sidebar tree. */
export interface TreeNode {
  id: TreeNodeId;
  label: string;
  children?: TreeNode[];
}

// ─── Kind Groupings ────────────────────────────────────────────────────────────

export const KIND_GROUPS = {
  Workloads: ["Pod", "Deployment", "StatefulSet", "DaemonSet", "Job", "CronJob"] as const,
  Network: ["Service", "Ingress"] as const,
  Config: ["ConfigMap", "Secret"] as const,
  Storage: ["PersistentVolumeClaim", "PersistentVolume", "StorageClass"] as const,
} as const;

export type KindGroupName = keyof typeof KIND_GROUPS;

// ─── Section Definitions ───────────────────────────────────────────────────────

export const SECTIONS = {
  /** Cluster Overview: cluster-scoped resources (Node, StorageClass, PersistentVolume) */
  clusterOverview: {
    id: "clusterOverview",
    label: "Cluster Overview",
    children: ["Node", "StorageClass", "PersistentVolume"] as const,
  },
  /** Namespaces: per-namespace browsing */
  namespaces: {
    id: "namespaces",
    label: "Namespaces",
    children: [] as readonly string[], // populated dynamically
  },
  /** Workloads: Pod, Deployment, StatefulSet, DaemonSet, Job, CronJob */
  workloads: {
    id: "workloads",
    label: "Workloads",
    children: [...KIND_GROUPS.Workloads] as const,
  },
  /** Network: Service, Ingress */
  network: {
    id: "network",
    label: "Network",
    children: [...KIND_GROUPS.Network] as const,
  },
  /** Config: ConfigMap, Secret */
  config: {
    id: "config",
    label: "Config",
    children: [...KIND_GROUPS.Config] as const,
  },
  /** Storage: PersistentVolumeClaim, PersistentVolume, StorageClass */
  storage: {
    id: "storage",
    label: "Storage",
    children: [...KIND_GROUPS.Storage] as const,
  },
  /** Argo CD: Applications, ApplicationSets, AppProjects */
  argo: {
    id: "argo",
    label: "Argo CD",
    children: ["Applications", "ApplicationSets", "AppProjects"] as const,
  },
} as const;

export type SectionName = keyof typeof SECTIONS;

// ─── Static Sections (always present, no namespace required) ──────────────────

/** Sections that are always shown regardless of namespace selection. */
export const STATIC_SECTION_NAMES: SectionName[] = [
  "clusterOverview",
  "workloads",
  "network",
  "config",
  "storage",
  "argo",
];

// ─── Section → Kinds Mapping ───────────────────────────────────────────────────

export function sectionKinds(section: SectionName): readonly string[] {
  return SECTIONS[section].children;
}

export function isNamespacedKind(kind: string): boolean {
  return (SUPPORTED_KINDS as readonly string[]).includes(kind) &&
    !(CLUSTER_SCOPED_KINDS as readonly string[]).includes(kind);
}

// ─── TreeNodeId Helpers ────────────────────────────────────────────────────────

export function nodeIdToString(id: TreeNodeId): string {
  const parts = [id.type, id.section];
  if (id.namespace) parts.push(id.namespace);
  if (id.group) parts.push(id.group);
  if (id.kind) parts.push(id.kind);
  return parts.join("::");
}

export function stringToNodeId(s: string): TreeNodeId {
  const parts = s.split("::");
  return {
    type: parts[0] as TreeNodeId["type"],
    section: parts[1],
    namespace: parts[2],
    group: parts[3],
    kind: parts[4],
  };
}

export function makeSectionNode(section: SectionName): TreeNode {
  return {
    id: { type: "section", section },
    label: SECTIONS[section].label,
  };
}

export function makeNamespaceNode(namespace: string): TreeNode {
  return {
    id: { type: "namespace", section: "namespaces", namespace },
    label: namespace,
  };
}

export function makeGroupNode(section: SectionName, namespace: string, group: KindGroupName): TreeNode {
  return {
    id: { type: "group", section, namespace, group },
    label: group,
  };
}

export function makeKindNode(section: SectionName, namespace: string, group: KindGroupName, kind: string): TreeNode {
  return {
    id: { type: "kind", section, namespace, group, kind },
    label: kind,
  };
}

// ─── Scope Resolution ───────────────────────────────────────────────────────────

export interface TreeScope {
  /** The selected section name (or null if root/cluster overview) */
  section: SectionName | "clusterOverview" | null;
  /** The selected namespace (null = cluster overview / no namespace selected) */
  namespace: string | null;
  /** The selected kind group (null if not in a group) */
  group: KindGroupName | null;
  /** The selected resource kind(s) */
  kinds: string[];
  /** Whether the scope is for cluster-scoped resources */
  clusterScoped: boolean;
  /** Whether the scope is for Argo CD */
  argoMode: boolean;
}

/**
 * Given a selected tree node, compute the full scope needed to drive ResourceList.
 * Returns kinds based on selection depth: section → group → kind.
 */
export function resolveTreeScope(nodeId: TreeNodeId | null): TreeScope {
  if (!nodeId) {
    return { section: null, namespace: null, group: null, kinds: [], clusterScoped: false, argoMode: false };
  }

  if (nodeId.type === "section") {
    if (nodeId.section === "clusterOverview") {
      return {
        section: "clusterOverview",
        namespace: null,
        group: null,
        kinds: [...CLUSTER_SCOPED_KINDS] as string[],
        clusterScoped: true,
        argoMode: false,
      };
    }
    // Section selected without namespace — no kinds yet
    return { section: nodeId.section as SectionName, namespace: null, group: null, kinds: [], clusterScoped: false, argoMode: nodeId.section === "argo" };
  }

  if (nodeId.type === "namespace") {
    // Namespace selected — return all namespaced kinds for that namespace
    const namespacedKinds = (SUPPORTED_KINDS as readonly string[]).filter(
      (k) => !(CLUSTER_SCOPED_KINDS as readonly string[]).includes(k)
    );
    return {
      section: "namespaces",
      namespace: nodeId.namespace ?? null,
      group: null,
      kinds: namespacedKinds,
      clusterScoped: false,
      argoMode: false,
    };
  }

  if (nodeId.type === "group") {
    const groupName = nodeId.group as KindGroupName;
    const groupKinds = KIND_GROUPS[groupName];
    if (!groupKinds) return { section: null, namespace: null, group: null, kinds: [], clusterScoped: false, argoMode: false };
    const kinds = [...groupKinds];
    return {
      section: nodeId.section as SectionName,
      namespace: nodeId.namespace ?? null,
      group: groupName,
      kinds,
      clusterScoped: false,
      argoMode: false,
    };
  }

  if (nodeId.type === "kind") {
    return {
      section: nodeId.section as SectionName,
      namespace: nodeId.namespace ?? null,
      group: nodeId.group as KindGroupName | null,
      kinds: nodeId.kind ? [nodeId.kind] : [],
      clusterScoped: nodeId.kind ? (CLUSTER_SCOPED_KINDS as readonly string[]).includes(nodeId.kind) : false,
      argoMode: nodeId.section === "argo",
    };
  }

  return { section: null, namespace: null, group: null, kinds: [], clusterScoped: false, argoMode: false };
}

// ─── Empty State Messages ─────────────────────────────────────────────────────

export function emptyStateMessage(scope: TreeScope, hasClusterContext: boolean): string {
  if (!hasClusterContext) return "Select a cluster context first";
  if (scope.argoMode) return "Select an Argo CD resource type";
  if (!scope.section) return "Select a section from the sidebar";
  if (scope.section === "clusterOverview" && scope.kinds.length > 0) return "Select a cluster context to view cluster-scoped resources";
  if (scope.section === "namespaces" && !scope.namespace) return "Select a namespace";
  if (scope.namespace && scope.kinds.length === 0) return "Select a resource kind";
  if (scope.kinds.length > 0 && !scope.clusterScoped && !scope.namespace) return "Select a namespace";
  return "Select a resource kind";
}

// ─── Argo Section Helpers ─────────────────────────────────────────────────────

export const ARGO_CHILDREN_LABELS: Record<string, string> = {
  Applications: "Applications",
  ApplicationSets: "ApplicationSets",
  AppProjects: "AppProjects",
};

export function isArgoSection(section: SectionName | string): boolean {
  return section === "argo";
}

export function argoChildKinds(child: string): string[] {
  switch (child) {
    case "Applications": return [];
    case "ApplicationSets": return [];
    case "AppProjects": return [];
    default: return [];
  }
}

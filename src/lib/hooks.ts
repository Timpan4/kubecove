import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { AnyKind, ResourceSummary, ArgoApplicationSummary } from "./types";
import type { TreeNodeId } from "./tree-nav";

export interface DashboardState {
  clusterContext: string;
  selectedNamespaces: string[];
  selectedKinds: AnyKind[];
  selectedResource: ResourceSummary | null;
  argoDetected: boolean;
  selectedArgoApp: ArgoApplicationSummary | null;
  viewMode: "resources" | "argo";
  // Tree navigation state
  selectedTreeNode: TreeNodeId | null;
  expandedSections: string[]; // array of nodeId strings for stable serialization
}

export interface DashboardSetters {
  setClusterContext: (ctx: string) => void;
  setSelectedNamespaces: (ns: string[]) => void;
  toggleNamespace: (ns: string) => void;
  setSelectedKinds: (kinds: AnyKind[]) => void;
  toggleKind: (kind: AnyKind) => void;
  setSelectedResource: (resource: ResourceSummary | null) => void;
  resetResource: () => void;
  setArgoDetected: (detected: boolean) => void;
  setSelectedArgoApp: (app: ArgoApplicationSummary | null) => void;
  setViewMode: (mode: "resources" | "argo") => void;
  // Tree navigation setters
  setSelectedTreeNode: (node: TreeNodeId | null) => void;
  setExpandedSections: (sections: string[]) => void;
  toggleExpandedSection: (nodeIdStr: string) => void;
}

// Persisted slice: context + namespaces only (Milestone 2 requirement)
interface PersistedSlice {
  clusterContext: string;
  selectedNamespaces: string[];
  setClusterContext: (ctx: string) => void;
  setSelectedNamespaces: (ns: string[]) => void;
  toggleNamespace: (ns: string) => void;
}

const usePersistedStore = create<PersistedSlice>()(
  persist(
    (set) => ({
      clusterContext: "",
      selectedNamespaces: [],
      setClusterContext: (ctx: string) => set({ clusterContext: ctx }),
      setSelectedNamespaces: (ns: string[]) => set({ selectedNamespaces: ns }),
      toggleNamespace: (ns: string) =>
        set((state) =>
          state.selectedNamespaces.includes(ns)
            ? { selectedNamespaces: state.selectedNamespaces.filter((n) => n !== ns) }
            : { selectedNamespaces: [...state.selectedNamespaces, ns] }
        ),
    }),
    { name: "k8s-manager-dashboard" }
  )
);

// Non-persisted slice: kinds + resource selection + Argo CD + tree navigation
interface NonPersistedSlice {
  selectedKinds: AnyKind[];
  selectedResource: ResourceSummary | null;
  setSelectedKinds: (kinds: AnyKind[]) => void;
  toggleKind: (kind: AnyKind) => void;
  setSelectedResource: (resource: ResourceSummary | null) => void;
  resetResource: () => void;
  // Argo CD state
  argoDetected: boolean;
  selectedArgoApp: ArgoApplicationSummary | null;
  setArgoDetected: (detected: boolean) => void;
  setSelectedArgoApp: (app: ArgoApplicationSummary | null) => void;
  // View mode: resources or argo
  viewMode: "resources" | "argo";
  setViewMode: (mode: "resources" | "argo") => void;
  // Tree navigation state
  selectedTreeNode: TreeNodeId | null;
  expandedSections: string[];
  setSelectedTreeNode: (node: TreeNodeId | null) => void;
  setExpandedSections: (sections: string[]) => void;
  toggleExpandedSection: (nodeIdStr: string) => void;
}

const useNonPersistedStore = create<NonPersistedSlice>()((set) => ({
  selectedKinds: [],
  selectedResource: null,
  setSelectedKinds: (kinds: AnyKind[]) => set({ selectedKinds: kinds }),
  toggleKind: (kind: AnyKind) =>
    set((state) =>
      state.selectedKinds.includes(kind)
        ? { selectedKinds: state.selectedKinds.filter((k) => k !== kind) }
        : { selectedKinds: [...state.selectedKinds, kind] }
    ),
  setSelectedResource: (resource: ResourceSummary | null) =>
    set({ selectedResource: resource }),
  resetResource: () => set({ selectedResource: null }),
  argoDetected: false,
  selectedArgoApp: null,
  setArgoDetected: (detected: boolean) => set({ argoDetected: detected }),
  setSelectedArgoApp: (app: ArgoApplicationSummary | null) => set({ selectedArgoApp: app }),
  viewMode: "resources",
  setViewMode: (mode: "resources" | "argo") => set({ viewMode: mode }),
  selectedTreeNode: null,
  expandedSections: [],
  setSelectedTreeNode: (node: TreeNodeId | null) => set({ selectedTreeNode: node }),
  setExpandedSections: (sections: string[]) => set({ expandedSections: sections }),
  toggleExpandedSection: (nodeIdStr: string) =>
    set((state) =>
      state.expandedSections.includes(nodeIdStr)
        ? { expandedSections: state.expandedSections.filter((s) => s !== nodeIdStr) }
        : { expandedSections: [...state.expandedSections, nodeIdStr] }
    ),
}));

export function useDashboardState(): DashboardState & DashboardSetters {
  const {
    clusterContext,
    setClusterContext,
    selectedNamespaces,
    setSelectedNamespaces,
    toggleNamespace,
  } = usePersistedStore();
  const {
    selectedKinds,
    setSelectedKinds,
    toggleKind,
    selectedResource,
    setSelectedResource,
    resetResource,
    argoDetected,
    selectedArgoApp,
    setArgoDetected,
    setSelectedArgoApp,
    viewMode,
    setViewMode,
    selectedTreeNode,
    expandedSections,
    setSelectedTreeNode,
    setExpandedSections,
    toggleExpandedSection,
  } = useNonPersistedStore();

  return {
    clusterContext,
    selectedNamespaces,
    selectedKinds,
    selectedResource,
    argoDetected,
    selectedArgoApp,
    viewMode,
    setClusterContext,
    setSelectedNamespaces,
    toggleNamespace,
    setSelectedKinds,
    toggleKind,
    setSelectedResource,
    resetResource,
    setArgoDetected,
    setSelectedArgoApp,
    setViewMode,
    selectedTreeNode,
    expandedSections,
    setSelectedTreeNode,
    setExpandedSections,
    toggleExpandedSection,
  };
}

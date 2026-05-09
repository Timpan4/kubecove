import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { SupportedKind, ResourceSummary } from "./types";

export interface DashboardState {
  clusterContext: string;
  selectedNamespaces: string[];
  selectedKinds: SupportedKind[];
  selectedResource: ResourceSummary | null;
}

export interface DashboardSetters {
  setClusterContext: (ctx: string) => void;
  setSelectedNamespaces: (ns: string[]) => void;
  toggleNamespace: (ns: string) => void;
  setSelectedKinds: (kinds: SupportedKind[]) => void;
  toggleKind: (kind: SupportedKind) => void;
  setSelectedResource: (resource: ResourceSummary | null) => void;
  resetResource: () => void;
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

// Non-persisted slice: kinds + resource selection
interface NonPersistedSlice {
  selectedKinds: SupportedKind[];
  selectedResource: ResourceSummary | null;
  setSelectedKinds: (kinds: SupportedKind[]) => void;
  toggleKind: (kind: SupportedKind) => void;
  setSelectedResource: (resource: ResourceSummary | null) => void;
  resetResource: () => void;
}

const useNonPersistedStore = create<NonPersistedSlice>()((set) => ({
  selectedKinds: [],
  selectedResource: null,
  setSelectedKinds: (kinds: SupportedKind[]) => set({ selectedKinds: kinds }),
  toggleKind: (kind: SupportedKind) =>
    set((state) =>
      state.selectedKinds.includes(kind)
        ? { selectedKinds: state.selectedKinds.filter((k) => k !== kind) }
        : { selectedKinds: [...state.selectedKinds, kind] }
    ),
  setSelectedResource: (resource: ResourceSummary | null) =>
    set({ selectedResource: resource }),
  resetResource: () => set({ selectedResource: null }),
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
  } = useNonPersistedStore();

  return {
    clusterContext,
    selectedNamespaces,
    selectedKinds,
    selectedResource,
    setClusterContext,
    setSelectedNamespaces,
    toggleNamespace,
    setSelectedKinds,
    toggleKind,
    setSelectedResource,
    resetResource,
  };
}

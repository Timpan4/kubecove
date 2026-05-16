import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
	ResourceSummary,
	ArgoApplicationSummary,
	ArgoApplicationSetSummary,
	ArgoAppProjectSummary,
	ResourceKindSelection,
} from "./types";
import type { TreeNodeId } from "./tree-nav";

export type ArgoSelectedItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary
	| null;

export interface DashboardState {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selectedResource: ResourceSummary | null;
	argoDetected: boolean;
	selectedArgoApp: ArgoSelectedItem;
	selectedArgoAppFilter: string;
	viewMode: "resources" | "argo" | "settings";
	// Tree navigation state
	selectedTreeNode: TreeNodeId | null;
	expandedSections: string[]; // array of nodeId strings for stable serialization
}

export interface DashboardSetters {
	setClusterContext: (ctx: string) => void;
	setSelectedNamespaces: (ns: string[]) => void;
	toggleNamespace: (ns: string) => void;
	setSelectedKinds: (kinds: ResourceKindSelection[]) => void;
	toggleKind: (kind: ResourceKindSelection) => void;
	setSelectedResource: (resource: ResourceSummary | null) => void;
	resetResource: () => void;
	setArgoDetected: (detected: boolean) => void;
	setSelectedArgoApp: (app: ArgoSelectedItem) => void;
	setSelectedArgoAppFilter: (app: string) => void;
	setViewMode: (mode: "resources" | "argo" | "settings") => void;
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
						? {
								selectedNamespaces: state.selectedNamespaces.filter(
									(n) => n !== ns,
								),
							}
						: { selectedNamespaces: [...state.selectedNamespaces, ns] },
				),
		}),
		{ name: "k8s-manager-dashboard" },
	),
);

// Non-persisted slice: kinds + resource selection + Argo CD + tree navigation
interface NonPersistedSlice {
	selectedKinds: ResourceKindSelection[];
	selectedResource: ResourceSummary | null;
	setSelectedKinds: (kinds: ResourceKindSelection[]) => void;
	toggleKind: (kind: ResourceKindSelection) => void;
	setSelectedResource: (resource: ResourceSummary | null) => void;
	resetResource: () => void;
	// Argo CD state
	argoDetected: boolean;
	selectedArgoApp: ArgoSelectedItem;
	selectedArgoAppFilter: string;
	setArgoDetected: (detected: boolean) => void;
	setSelectedArgoApp: (app: ArgoSelectedItem) => void;
	setSelectedArgoAppFilter: (app: string) => void;
	// View mode: resources or argo
	viewMode: "resources" | "argo" | "settings";
	setViewMode: (mode: "resources" | "argo" | "settings") => void;
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
	setSelectedKinds: (kinds: ResourceKindSelection[]) => set({ selectedKinds: kinds }),
	toggleKind: (kind: ResourceKindSelection) =>
		set((state) =>
			state.selectedKinds.includes(kind)
				? { selectedKinds: state.selectedKinds.filter((k) => k !== kind) }
				: { selectedKinds: [...state.selectedKinds, kind] },
		),
	setSelectedResource: (resource: ResourceSummary | null) =>
		set({ selectedResource: resource }),
	resetResource: () => set({ selectedResource: null }),
	argoDetected: false,
	selectedArgoApp: null,
	selectedArgoAppFilter: "",
	setArgoDetected: (detected: boolean) => set({ argoDetected: detected }),
	setSelectedArgoApp: (app: ArgoSelectedItem) => set({ selectedArgoApp: app }),
	setSelectedArgoAppFilter: (app: string) => set({ selectedArgoAppFilter: app }),
	viewMode: "resources",
	setViewMode: (mode: "resources" | "argo" | "settings") => set({ viewMode: mode }),
	selectedTreeNode: null,
	expandedSections: [],
	setSelectedTreeNode: (node: TreeNodeId | null) =>
		set({ selectedTreeNode: node }),
	setExpandedSections: (sections: string[]) =>
		set({ expandedSections: sections }),
	toggleExpandedSection: (nodeIdStr: string) =>
		set((state) =>
			state.expandedSections.includes(nodeIdStr)
				? {
						expandedSections: state.expandedSections.filter(
							(s) => s !== nodeIdStr,
						),
					}
				: { expandedSections: [...state.expandedSections, nodeIdStr] },
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
		selectedArgoAppFilter,
		setArgoDetected,
		setSelectedArgoApp,
		setSelectedArgoAppFilter,
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
		selectedArgoAppFilter,
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
		setSelectedArgoAppFilter,
		setViewMode,
		selectedTreeNode,
		expandedSections,
		setSelectedTreeNode,
		setExpandedSections,
		toggleExpandedSection,
	};
}

import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
	ResourceSummary,
	ArgoApplicationSummary,
	ArgoApplicationSetSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
	HelmReleaseSummary,
	ResourceKindSelection,
} from "./types";
import type { TreeNodeId } from "./tree-nav";
import type { HealthFilter } from "../features/resources/helpers";

export type ArgoSelectedItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary
	| null;

export type InspectorSelection =
	| { type: "resource"; resource: ResourceSummary }
	| { type: "argo"; app: NonNullable<ArgoSelectedItem> }
	| { type: "flux"; resource: FluxResourceSummary }
	| { type: "helm"; release: HelmReleaseSummary }
	| null;

export interface OpenViewOptions {
	treeNode?: TreeNodeId | null;
	argoAppFilter?: string;
	initialSearch?: string;
	healthFilter?: HealthFilter;
	preserveSelection?: boolean;
	preserveResourceFilters?: boolean;
}

export interface DashboardState {
	clusterContext: string;
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	selection: InspectorSelection;
	selectedResource: ResourceSummary | null;
	argoDetected: boolean;
	selectedArgoApp: ArgoSelectedItem;
	selectedFluxResource: FluxResourceSummary | null;
	selectedHelmRelease: HelmReleaseSummary | null;
	selectedArgoAppFilter: string;
	resourceInitialSearch: string;
	resourceHealthFilter: HealthFilter;
	viewMode: DashboardViewMode;
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
	select: (selection: InspectorSelection) => void;
	setArgoDetected: (detected: boolean) => void;
	setSelectedArgoAppFilter: (app: string) => void;
	setViewMode: (mode: DashboardViewMode) => void;
	openView: (mode: DashboardViewMode, options?: OpenViewOptions) => void;
	setResourceInitialSearch: (search: string) => void;
	// Tree navigation setters
	setSelectedTreeNode: (node: TreeNodeId | null) => void;
	setExpandedSections: (sections: string[]) => void;
	toggleExpandedSection: (nodeIdStr: string) => void;
}

export type DashboardViewMode =
	| "overview"
	| "resources"
	| "argo"
	| "helm"
	| "incidents"
	| "portForwards"
	| "rbac"
	| "settings";

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
		{ name: "kubecove-dashboard" },
	),
);

// Non-persisted slice: view state + inspector selection + tree navigation
interface NonPersistedSlice {
	selectedKinds: ResourceKindSelection[];
	selection: InspectorSelection;
	setSelectedKinds: (kinds: ResourceKindSelection[]) => void;
	toggleKind: (kind: ResourceKindSelection) => void;
	select: (selection: InspectorSelection) => void;
	// Argo CD state
	argoDetected: boolean;
	selectedArgoAppFilter: string;
	setArgoDetected: (detected: boolean) => void;
	setSelectedArgoAppFilter: (app: string) => void;
	resourceInitialSearch: string;
	resourceHealthFilter: HealthFilter;
	viewMode: DashboardViewMode;
	setViewMode: (mode: DashboardViewMode) => void;
	openView: (mode: DashboardViewMode, options?: OpenViewOptions) => void;
	setResourceInitialSearch: (search: string) => void;
	// Tree navigation state
	selectedTreeNode: TreeNodeId | null;
	expandedSections: string[];
	setSelectedTreeNode: (node: TreeNodeId | null) => void;
	setExpandedSections: (sections: string[]) => void;
	toggleExpandedSection: (nodeIdStr: string) => void;
}

function resourceKindKey(kind: ResourceKindSelection): string {
	if (typeof kind === "string") return kind;
	return `${kind.group}/${kind.version}/${kind.kind}/${kind.plural}`;
}

export const useDashboardStore = create<NonPersistedSlice>()((set) => ({
	selectedKinds: [],
	selection: null,
	setSelectedKinds: (kinds: ResourceKindSelection[]) => set({ selectedKinds: kinds }),
	toggleKind: (kind: ResourceKindSelection) =>
		set((state) => {
			const key = resourceKindKey(kind);
			const selected = state.selectedKinds.some(
				(selectedKind) => resourceKindKey(selectedKind) === key,
			);
			return selected
				? {
						selectedKinds: state.selectedKinds.filter(
							(selectedKind) => resourceKindKey(selectedKind) !== key,
						),
					}
				: { selectedKinds: [...state.selectedKinds, kind] };
		}),
	select: (selection: InspectorSelection) => set({ selection }),
	argoDetected: false,
	selectedArgoAppFilter: "",
	setArgoDetected: (detected: boolean) => set({ argoDetected: detected }),
	setSelectedArgoAppFilter: (app: string) => set({ selectedArgoAppFilter: app }),
	resourceInitialSearch: "",
	resourceHealthFilter: "all",
	viewMode: "resources",
	setViewMode: (mode: DashboardViewMode) => set({ viewMode: mode }),
	openView: (mode: DashboardViewMode, options?: OpenViewOptions) =>
		set((state) => ({
			viewMode: mode,
			selection: options?.preserveSelection ? state.selection : null,
			resourceInitialSearch: options?.preserveResourceFilters
				? state.resourceInitialSearch
				: options?.initialSearch ?? "",
			resourceHealthFilter: options?.preserveResourceFilters
				? state.resourceHealthFilter
				: options?.healthFilter ?? "all",
			selectedArgoAppFilter: options?.preserveResourceFilters
				? state.selectedArgoAppFilter
				: options?.argoAppFilter ?? "",
			...(options?.treeNode !== undefined
				? { selectedTreeNode: options.treeNode }
				: {}),
		})),
	setResourceInitialSearch: (search: string) =>
		set({ resourceInitialSearch: search }),
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
		selection,
		select,
		argoDetected,
		selectedArgoAppFilter,
		setArgoDetected,
		setSelectedArgoAppFilter,
		resourceInitialSearch,
		resourceHealthFilter,
		viewMode,
		setViewMode,
		openView,
		setResourceInitialSearch,
		selectedTreeNode,
		expandedSections,
		setSelectedTreeNode,
		setExpandedSections,
		toggleExpandedSection,
	} = useDashboardStore();

	return {
		clusterContext,
		selectedNamespaces,
		selectedKinds,
		selection,
		selectedResource:
			selection?.type === "resource" ? selection.resource : null,
		argoDetected,
		selectedArgoApp: selection?.type === "argo" ? selection.app : null,
		selectedFluxResource:
			selection?.type === "flux" ? selection.resource : null,
		selectedHelmRelease: selection?.type === "helm" ? selection.release : null,
		selectedArgoAppFilter,
		resourceInitialSearch,
		resourceHealthFilter,
		viewMode,
		setClusterContext,
		setSelectedNamespaces,
		toggleNamespace,
		setSelectedKinds,
		toggleKind,
		select,
		setArgoDetected,
		setSelectedArgoAppFilter,
		setViewMode,
		openView,
		setResourceInitialSearch,
		selectedTreeNode,
		expandedSections,
		setSelectedTreeNode,
		setExpandedSections,
		toggleExpandedSection,
	};
}

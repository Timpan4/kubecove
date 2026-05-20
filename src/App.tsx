import "./App.css";
import {
	lazy,
	Suspense,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useDashboardState } from "./lib/hooks";
import { SidebarTree } from "./components/SidebarTree";
import { AppTopBar } from "./app/AppTopBar";
import { AppUsageFooter } from "./app/AppUsageFooter";
import { DetailPanelFrame } from "./app/DetailPanelFrame";
import { useArgoDetection } from "./app/useArgoDetection";
import { ViewLoadingFallback } from "./app/ViewLoadingFallback";
import { useSettingsState } from "./lib/settings";
import {
	Sidebar,
	SidebarContent,
	SidebarInset,
	SidebarProvider,
} from "@/components/ui/sidebar";
import {
	emptyStateMessage,
	resolveTreeScope,
	type TreeNodeId,
} from "./lib/tree-nav";
import { diagnosticLog } from "./lib/diagnostics";
import type { ResourceKindSelection } from "./lib/types";
import type { HealthFilter } from "./features/resources/helpers";
import {
	makeWorkspaceShortcuts,
	useWorkspaceStore,
	type SavedWorkspace,
} from "./lib/workspaces";
import {
	hasDiscoveredKind,
	resourceKindLabel,
	resourceKindLogKey,
	SECTION_LABELS,
	SIDEBAR_PROVIDER_STYLE,
} from "./app/viewHelpers";

const ResourceList = lazy(() =>
	import("./features/resources/ResourceList").then((module) => ({
		default: module.ResourceList,
	})),
);
const ResourceDetailPanel = lazy(() =>
	import("./features/resource-detail/ResourceDetailPanel").then((module) => ({
		default: module.ResourceDetailPanel,
	})),
);
const ArgoCDPanel = lazy(() =>
	import("./features/argo/ArgoCDPanel").then((module) => ({
		default: module.ArgoCDPanel,
	})),
);
const ArgoDetailPanel = lazy(() =>
	import("./features/argo/ArgoDetailPanel").then((module) => ({
		default: module.ArgoDetailPanel,
	})),
);
const SettingsPage = lazy(() =>
	import("./features/settings/SettingsPage").then((module) => ({
		default: module.SettingsPage,
	})),
);
const WorkspaceLauncher = lazy(() =>
	import("./features/workspaces").then((module) => ({
		default: module.WorkspaceLauncher,
	})),
);
const WorkspaceOverview = lazy(() =>
	import("./features/workspaces").then((module) => ({
		default: module.WorkspaceOverview,
	})),
);

function App() {
	const {
		workspaces,
		activeWorkspaceId,
		setActiveWorkspace,
		updateWorkspace,
	} = useWorkspaceStore();
	const {
		clusterContext,
		selectedNamespaces,
		selectedKinds,
		selectedResource,
		selectedArgoApp,
		selectedArgoAppFilter,
		viewMode,
		setClusterContext,
		setSelectedNamespaces,
		setSelectedKinds,
		setSelectedResource,
		resetResource,
		setArgoDetected,
		setSelectedArgoApp,
		setSelectedArgoAppFilter,
		setViewMode,
		selectedTreeNode,
		expandedSections,
		setSelectedTreeNode,
		toggleExpandedSection,
	} = useDashboardState();
	const activeWorkspace =
		workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null;
	const showUsageFooter = useSettingsState((state) => state.showUsageFooter);
	const [resourceHealthFilter, setResourceHealthFilter] =
		useState<HealthFilter>("all");
	const appRenderCountRef = useRef(0);
	appRenderCountRef.current += 1;

	const applyWorkspace = useCallback(
		(workspace: SavedWorkspace) => {
			setActiveWorkspace(workspace.id);
			setClusterContext(workspace.scope.clusterContext);
			setSelectedNamespaces(workspace.scope.namespaces);
			setSelectedKinds(workspace.scope.kinds);
			setSelectedResource(null);
			setSelectedArgoApp(null);
			setSelectedArgoAppFilter(workspace.scope.argoAppFilter);
			setSelectedTreeNode(null);
			setResourceHealthFilter("all");
			setViewMode("overview");
		},
		[
			setActiveWorkspace,
			setClusterContext,
			setSelectedNamespaces,
			setSelectedKinds,
			setSelectedResource,
			setSelectedArgoApp,
			setSelectedArgoAppFilter,
			setSelectedTreeNode,
			setViewMode,
		],
	);

	const handleClusterChange = (ctx: string) => {
		diagnosticLog("app.cluster.change", { cluster: ctx });
		setClusterContext(ctx);
		// Clear inspector state on context switch
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedArgoAppFilter("");
		setSelectedNamespaces([]);
		setSelectedKinds(activeWorkspace?.scope.kinds ?? []);
		setArgoDetected(false);
		setSelectedTreeNode(null);
		setResourceHealthFilter("all");
		setViewMode("resources");
		if (activeWorkspace) {
			updateWorkspace(activeWorkspace.id, {
				scope: {
					...activeWorkspace.scope,
					clusterContext: ctx,
					namespaces: [],
					argoAppFilter: "",
				},
				shortcuts: makeWorkspaceShortcuts([]),
			});
		}
	};

	const handleOpenResources = (
		namespace?: string,
		healthFilter: HealthFilter = "all",
	) => {
		const workspace = activeWorkspace;
		if (!workspace) return;
		setSelectedNamespaces(
			namespace ? [namespace] : workspace.scope.namespaces,
		);
		setSelectedKinds(workspace.scope.kinds);
		setSelectedArgoAppFilter(workspace.scope.argoAppFilter);
		setSelectedTreeNode(null);
		setSelectedArgoApp(null);
		setSelectedResource(null);
		setResourceHealthFilter(healthFilter);
		setViewMode("resources");
	};

	const handleOpenArgo = (argoApp?: string) => {
		setSelectedArgoAppFilter(argoApp ?? "");
		setSelectedTreeNode({ type: "section", section: "argo" });
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setResourceHealthFilter("all");
		setViewMode("argo");
	};

	const handleOpenLauncher = () => {
		setActiveWorkspace(null);
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedTreeNode(null);
		setResourceHealthFilter("all");
		setViewMode("resources");
	};

	const handleTreeNodeSelect = (nodeId: TreeNodeId) => {
		const scope = resolveTreeScope(nodeId);
		diagnosticLog("app.tree.select", {
			type: nodeId.type,
			section: nodeId.section ?? "",
			namespace: nodeId.namespace ?? "",
			kind: nodeId.kind ?? "",
			argoMode: scope.argoMode,
		});

		// Argo section or child → switch to argo view, clear resource state
		setSelectedArgoAppFilter("");

		if (scope.argoMode) {
			setViewMode("argo");
			setSelectedArgoApp(null);
			setSelectedResource(null);
			setResourceHealthFilter("all");
		} else if (
			viewMode === "argo" ||
			viewMode === "settings" ||
			viewMode === "overview"
		) {
			// Leaving non-resource views clears inspector state.
			setViewMode("resources");
			setSelectedArgoApp(null);
			setResourceHealthFilter("all");
		}

		setSelectedTreeNode(nodeId);
	};

	const selectedResourceKey = useMemo(
		() =>
			selectedResource
				? `${selectedResource.cluster}::${selectedResource.apiVersion ?? ""}::${selectedResource.kind}::${selectedResource.namespace ?? ""}::${selectedResource.name}`
				: null,
		[
			selectedResource?.cluster,
			selectedResource?.apiVersion,
			selectedResource?.kind,
			selectedResource?.namespace,
			selectedResource?.name,
		],
	);

	const handleArgoAppSelect = (
		app: NonNullable<ReturnType<typeof useDashboardState>["selectedArgoApp"]>,
	) => {
		diagnosticLog("app.argo.select", {
			name: app.name,
			namespace: app.namespace ?? "",
		});
		setSelectedArgoApp(app);
	};

	const handleArgoClose = () => {
		diagnosticLog("app.argo.close");
		setSelectedArgoApp(null);
	};

	useArgoDetection(clusterContext, setArgoDetected);

	// Compute scope from selected tree node
	const scope = useMemo(
		() => resolveTreeScope(selectedTreeNode),
		[selectedTreeNode],
	);

	const computedKinds = useMemo<ResourceKindSelection[]>(() => {
		if (scope.kinds.length > 0) return scope.kinds;
		return selectedKinds as ResourceKindSelection[];
	}, [scope.kinds, selectedKinds]);

	const computedNamespaces = useMemo<string[]>(() => {
		if (scope.namespace) return [scope.namespace];
		return selectedNamespaces;
	}, [scope.namespace, selectedNamespaces]);

	const contentTitle = useMemo(() => {
		if (viewMode === "overview") return activeWorkspace?.name ?? "Workspace";
		if (viewMode === "settings") return "Settings";
		if (viewMode === "argo") {
			if (selectedTreeNode?.type === "kind" && selectedTreeNode.kind) {
				return `${selectedTreeNode.kind}`;
			}
			return "Argo CD";
		}
		if (!scope.section) return "Kubernetes Resources";
		if (scope.section === "clusterOverview") {
			if (scope.kinds.length === 1) return `${resourceKindLabel(scope.kinds[0])} Resources`;
			if (scope.kinds.length > 1) return "Cluster Overview";
			return "Cluster Overview";
		}
		if (scope.section === "namespaces" && scope.namespace) {
			if (scope.group && scope.kinds.length > 0) {
				return `${scope.namespace} / ${scope.group}`;
			}
			return scope.namespace;
		}
		if (scope.group) return scope.group;
		if (scope.kinds.length === 1) return `${resourceKindLabel(scope.kinds[0])} Resources`;
		if (scope.kinds.length > 1)
			return SECTION_LABELS[scope.section] ?? scope.section;
		return SECTION_LABELS[scope.section] ?? scope.section;
	}, [scope, viewMode, selectedTreeNode, activeWorkspace?.name]);

	const canQueryResources =
		computedKinds.length > 0 &&
		!!clusterContext &&
		(scope.clusterScoped ||
			computedNamespaces.length > 0 ||
			hasDiscoveredKind(computedKinds));

	const emptyMsg = useMemo(
		() => emptyStateMessage(scope, !!clusterContext),
		[scope, clusterContext],
	);

	useEffect(() => {
		diagnosticLog("app.render", {
			render: appRenderCountRef.current,
			cluster: clusterContext,
			view: viewMode,
			canQuery: canQueryResources,
			kinds: computedKinds.map(resourceKindLogKey).join("|"),
			namespaces: computedNamespaces.join("|"),
			selectedResource: selectedResourceKey ?? "",
			argoFilter: selectedArgoAppFilter,
		});
	});

	if (!activeWorkspace) {
		return (
			<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
				<div className="min-h-0 flex-1 overflow-hidden">
					<Suspense fallback={<ViewLoadingFallback label="Loading workspaces..." />}>
						<WorkspaceLauncher onOpenWorkspace={applyWorkspace} />
					</Suspense>
				</div>
				<AppUsageFooter visible={showUsageFooter} />
			</div>
		);
	}

	const mainContent = (
		<div
			role="main"
			className="flex h-full w-full min-w-0 flex-col overflow-hidden"
		>
			{viewMode === "overview" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading overview..." />}>
						<WorkspaceOverview
							workspace={activeWorkspace}
							onOpenResources={handleOpenResources}
							onOpenArgo={handleOpenArgo}
							onOpenLauncher={handleOpenLauncher}
						/>
					</Suspense>
				</div>
			) : viewMode === "settings" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading settings..." />}>
						<SettingsPage />
					</Suspense>
				</div>
			) : viewMode === "argo" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading Argo CD..." />}>
						<ArgoCDPanel
							clusterContext={clusterContext}
							selectedArgoItem={selectedArgoApp}
							onArgoItemSelect={handleArgoAppSelect}
							selectedArgoKind={
								selectedTreeNode?.type === "kind" && selectedTreeNode.kind
									? selectedTreeNode.kind
									: null
							}
						/>
					</Suspense>
				</div>
			) : (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					{canQueryResources ? (
						<Suspense fallback={<ViewLoadingFallback label="Loading resources..." />}>
							<ResourceList
								clusterContext={clusterContext}
								selectedNamespaces={computedNamespaces}
								selectedKinds={computedKinds}
								selectedArgoAppFilter={selectedArgoAppFilter}
								selectedResource={selectedResource}
								initialHealthFilter={resourceHealthFilter}
								onArgoAppFilterChange={setSelectedArgoAppFilter}
								onResourceSelect={setSelectedResource}
							/>
						</Suspense>
					) : (
						<div className="p-8 text-center text-sm text-muted-foreground">
							{emptyMsg}
						</div>
					)}
				</div>
			)}
		</div>
	);

	const detailPanel =
		viewMode === "argo" && selectedArgoApp ? (
			<Suspense fallback={<ViewLoadingFallback label="Loading app details..." />}>
				<ArgoDetailPanel app={selectedArgoApp} onClose={handleArgoClose} />
			</Suspense>
		) : selectedResource ? (
			<Suspense fallback={<ViewLoadingFallback label="Loading resource details..." />}>
				<ResourceDetailPanel
					key={selectedResourceKey}
					resource={selectedResource}
					onClose={resetResource}
				/>
			</Suspense>
		) : null;

	return (
		<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<AppTopBar
				clusterContext={clusterContext}
				contentTitle={contentTitle}
				onClusterChange={handleClusterChange}
				onOpenLauncher={handleOpenLauncher}
				onOpenSettings={() => {
					setViewMode("settings");
					setSelectedResource(null);
					setSelectedArgoApp(null);
				}}
			/>

			<SidebarProvider
				defaultOpen
				className="min-h-0 flex-1 overflow-hidden"
				style={SIDEBAR_PROVIDER_STYLE}
			>
				<Sidebar
					collapsible="none"
					className="shrink-0 border-r bg-sidebar"
				>
					<SidebarContent className="overflow-y-auto overflow-x-hidden">
					<SidebarTree
						clusterContext={clusterContext}
						selectedNode={selectedTreeNode}
						expandedSections={expandedSections}
						onNodeSelect={handleTreeNodeSelect}
						onSectionToggle={toggleExpandedSection}
					/>
					</SidebarContent>
				</Sidebar>

				<SidebarInset className="min-h-0 min-w-0 flex-1 overflow-hidden">
				<DetailPanelFrame
					mainContent={mainContent}
					detailPanel={detailPanel}
				/>
				</SidebarInset>
			</SidebarProvider>
			<AppUsageFooter visible={showUsageFooter} />
		</div>
	);
}

export default App;

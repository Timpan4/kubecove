import "./App.css";
import { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDashboardState } from "./lib/hooks";
import { SidebarTree } from "./components/SidebarTree";
import { AppTopBar } from "./app/AppTopBar";
import { AppUsageFooter } from "./app/AppUsageFooter";
import { DetailPanelFrame } from "./app/DetailPanelFrame";
import {
	ArgoCDPanel,
	ArgoDetailPanel,
	HelmDetailPanel,
	HelmPanel,
	RbacPanel,
	ResourceDetailPanel,
	ResourceList,
	SettingsPage,
	WorkspaceLauncher,
	WorkspaceOverview,
	WorkspacePortForwardsPage,
} from "./app/lazyViews";
import { useArgoDetection } from "./app/useArgoDetection";
import { ViewLoadingFallback } from "./app/ViewLoadingFallback";
import { useAppUpdateLaunchCheck } from "./features/app-updates/useAppUpdateLaunchCheck";
import { SavedPortForwardRestorePrompt } from "./features/live-sessions";
import {
	shouldAutoStartSavedPortForwards,
	shouldShowSavedPortForwardRestorePrompt,
} from "./features/live-sessions/restore";
import { useSavedPortForwardActions } from "./features/live-sessions/useSavedPortForwardActions";
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
import {
	SUPPORTED_KINDS,
	type HelmReleaseSummary,
	type ResourceKindSelection,
} from "./lib/types";
import type { HealthFilter } from "./features/resources/helpers";
import {
	createWorkspaceScope,
	makeWorkspaceShortcuts,
	useWorkspaceStore,
	type SavedWorkspace,
} from "./lib/workspaces";
import {
	canQueryResourceScope,
	getAppContentTitle,
	resourceKindLogKey,
	SIDEBAR_PROVIDER_STYLE,
} from "./app/viewHelpers";

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
	const autoStartSavedPortForwards = useSettingsState(
		(state) => state.autoStartSavedPortForwards,
	);
	const [resourceHealthFilter, setResourceHealthFilter] =
		useState<HealthFilter>("all");
	const [selectedHelmRelease, setSelectedHelmRelease] =
		useState<HelmReleaseSummary | null>(null);
	const [targetHelmRelease, setTargetHelmRelease] = useState<{
		name: string;
		namespace?: string;
	} | null>(null);
	const [resourceInitialSearch, setResourceInitialSearch] = useState("");
	const [
		dismissedPortForwardRestoreWorkspaceId,
		setDismissedPortForwardRestoreWorkspaceId,
	] = useState<string | null>(null);
	const autoStartedSavedPortForwardWorkspaceIdsRef = useRef<Set<string>>(
		new Set(),
	);
	const savedPortForwardActions = useSavedPortForwardActions(activeWorkspace);
	const appRenderCountRef = useRef(0);
	appRenderCountRef.current += 1;
	useAppUpdateLaunchCheck();

	const applyWorkspace = useCallback(
		(workspace: SavedWorkspace) => {
			setActiveWorkspace(workspace.id);
			setClusterContext(workspace.scope.clusterContext);
			setSelectedNamespaces(workspace.scope.namespaces);
			setSelectedKinds(workspace.scope.kinds);
			setSelectedResource(null);
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setResourceInitialSearch("");
			setSelectedArgoAppFilter(workspace.scope.argoAppFilter);
			setSelectedTreeNode(null);
			setResourceHealthFilter("all");
			setDismissedPortForwardRestoreWorkspaceId(null);
			autoStartedSavedPortForwardWorkspaceIdsRef.current.delete(workspace.id);
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
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setSelectedArgoAppFilter("");
		setSelectedNamespaces([]);
		setSelectedKinds(activeWorkspace?.scope.kinds ?? []);
		setArgoDetected(false);
		setSelectedTreeNode(null);
		setResourceHealthFilter("all");
		setViewMode("resources");
		if (activeWorkspace) {
			const scope = createWorkspaceScope({
				name: activeWorkspace.name,
				clusterContext: ctx,
				clusterContexts: [ctx],
				namespaces: [],
				kinds: activeWorkspace.scope.kinds,
				shortcutPreferences: activeWorkspace.scope.shortcutPreferences,
			});
			updateWorkspace(activeWorkspace.id, {
				scope,
				shortcuts: makeWorkspaceShortcuts(
					scope.namespaces,
					undefined,
					scope.shortcutPreferences,
					scope,
				),
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
		setSelectedHelmRelease(null);
		setSelectedResource(null);
		setResourceInitialSearch("");
		setResourceHealthFilter(healthFilter);
		setViewMode("resources");
	};

	const handleOpenArgo = (argoApp?: string) => {
		setSelectedArgoAppFilter(argoApp ?? "");
		setSelectedTreeNode({ type: "section", section: "argo" });
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setResourceHealthFilter("all");
		setViewMode("argo");
	};

	const handleOpenLauncher = () => {
		setActiveWorkspace(null);
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setSelectedTreeNode(null);
		setResourceHealthFilter("all");
		setViewMode("resources");
	};

	const handleOpenSettings = () => {
		setViewMode("settings");
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
	};

	const handleOpenPortForwards = () => {
		setViewMode("portForwards");
		setSelectedTreeNode({ type: "section", section: "portForwards" });
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setResourceHealthFilter("all");
		if (activeWorkspace) {
			setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
		}
	};

	const handleTreeNodeSelect = (nodeId: TreeNodeId) => {
		const scope = resolveTreeScope(nodeId);
		diagnosticLog("app.tree.select", {
			type: nodeId.type,
			section: nodeId.section ?? "",
			namespace: nodeId.namespace ?? "",
			kind: nodeId.kind ?? "",
			argoMode: scope.argoMode,
			helmMode: scope.helmMode,
			portForwardMode: scope.portForwardMode,
			rbacMode: scope.rbacMode,
		});

		// Tool sections own their inspector state.
		setSelectedArgoAppFilter("");

		if (scope.argoMode) {
			setViewMode("argo");
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.helmMode) {
			setViewMode("helm");
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.portForwardMode) {
			setViewMode("portForwards");
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
			if (activeWorkspace) {
				setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
			}
		} else if (scope.rbacMode) {
			setViewMode("rbac");
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (
			viewMode === "argo" ||
			viewMode === "helm" ||
			viewMode === "portForwards" ||
			viewMode === "rbac" ||
			viewMode === "settings" ||
			viewMode === "overview"
		) {
			// Leaving non-resource views clears inspector state.
			setViewMode("resources");
			setSelectedArgoApp(null);
			setSelectedHelmRelease(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		}
		if (
			!scope.argoMode &&
			!scope.helmMode &&
			!scope.portForwardMode &&
			!scope.rbacMode
		) {
			setResourceInitialSearch("");
		}

		setSelectedTreeNode(nodeId);
	};

	const selectedResourceKey = selectedResource
		? `${selectedResource.cluster}::${selectedResource.apiVersion ?? ""}::${selectedResource.kind}::${selectedResource.namespace ?? ""}::${selectedResource.name}`
		: null;

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

	const handleHelmReleaseSelect = (release: HelmReleaseSummary) => {
		diagnosticLog("app.helm.select", {
			name: release.name,
			namespace: release.namespace,
		});
		setSelectedHelmRelease(release);
	};

	const handleHelmClose = () => {
		diagnosticLog("app.helm.close");
		setSelectedHelmRelease(null);
	};

	const handleOpenHelmResources = (release: HelmReleaseSummary) => {
		diagnosticLog("app.helm.openResources", {
			name: release.name,
			namespace: release.namespace,
		});
		setSelectedNamespaces([release.namespace]);
		setSelectedKinds([...SUPPORTED_KINDS]);
		setSelectedArgoAppFilter("");
		setSelectedTreeNode(null);
		setSelectedArgoApp(null);
		setSelectedResource(null);
		setResourceInitialSearch(release.name);
		setResourceHealthFilter("all");
		setViewMode("resources");
	};

	const handleOpenHelmReleaseFromResource = (
		releaseName: string,
		namespace?: string | null,
	) => {
		diagnosticLog("app.resource.openHelmRelease", {
			name: releaseName,
			namespace: namespace ?? "",
		});
		setTargetHelmRelease({
			name: releaseName,
			namespace: namespace ?? undefined,
		});
		setSelectedHelmRelease(null);
		setSelectedResource(null);
		setViewMode("helm");
	};

	const handleTargetHelmReleaseResolved = useCallback(() => {
		setTargetHelmRelease(null);
	}, [setTargetHelmRelease]);

	const handleResourceNamespacesChange = useCallback(
		(namespaces: string[]) => {
			setSelectedTreeNode(null);
			setSelectedResource(null);
			setSelectedNamespaces(namespaces);
		},
		[setSelectedTreeNode, setSelectedResource, setSelectedNamespaces],
	);

	const handleResourceKindsChange = useCallback(
		(kinds: ResourceKindSelection[]) => {
			setSelectedTreeNode(null);
			setSelectedResource(null);
			setSelectedKinds(kinds);
		},
		[setSelectedTreeNode, setSelectedResource, setSelectedKinds],
	);

	useArgoDetection(clusterContext, setArgoDetected);

	useEffect(() => {
		if (
			!shouldAutoStartSavedPortForwards({
				workspace: activeWorkspace,
				autoStart: autoStartSavedPortForwards,
				startedWorkspaceIds:
					autoStartedSavedPortForwardWorkspaceIdsRef.current,
			}) ||
			!activeWorkspace
		) {
			return;
		}
		autoStartedSavedPortForwardWorkspaceIdsRef.current.add(activeWorkspace.id);
		void savedPortForwardActions.startAll();
	}, [
		activeWorkspace,
		autoStartSavedPortForwards,
		savedPortForwardActions,
	]);

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
		if (scope.section === "namespaces") return [];
		return selectedNamespaces;
	}, [scope.namespace, scope.section, selectedNamespaces]);

	const contentTitle = useMemo(
		() =>
			getAppContentTitle({
				activeWorkspace,
				scope,
				selectedTreeNode,
				viewMode,
			}),
		[activeWorkspace, scope, selectedTreeNode, viewMode],
	);

	const canQueryResources = canQueryResourceScope({
		clusterContext,
		kinds: computedKinds,
		namespaces: computedNamespaces,
		scope,
		hasActiveWorkspace: activeWorkspace !== null,
	});

	const emptyMsg = useMemo(
		() => emptyStateMessage(scope, !!clusterContext),
		[scope, clusterContext],
	);
	const showPortForwardRestorePrompt =
		shouldShowSavedPortForwardRestorePrompt({
			workspace: activeWorkspace,
			autoStart: autoStartSavedPortForwards,
			dismissedWorkspaceId: dismissedPortForwardRestoreWorkspaceId,
		});

	useEffect(() => {
		diagnosticLog("app.render", {
			render: appRenderCountRef.current,
			cluster: clusterContext,
			view: viewMode,
			canQuery: canQueryResources,
			kinds: computedKinds.map(resourceKindLogKey).join("|"),
			namespaces: computedNamespaces.join("|"),
			selectedResource: selectedResourceKey ?? "",
			selectedHelmRelease: selectedHelmRelease?.name ?? "",
			argoFilter: selectedArgoAppFilter,
		});
	});

	if (!activeWorkspace) {
		return (
			<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
				<AppTopBar
					clusterContext={clusterContext}
					contentTitle={viewMode === "settings" ? "Settings" : "Workspaces"}
					onClusterChange={handleClusterChange}
					onOpenLauncher={handleOpenLauncher}
					onOpenSettings={handleOpenSettings}
					showClusterSelector={false}
					showSearch={false}
				/>
				<div className="min-h-0 flex-1 overflow-hidden">
					{viewMode === "settings" ? (
						<div className="h-full overflow-y-auto overflow-x-hidden p-4 md:px-6">
							<Suspense fallback={<ViewLoadingFallback label="Loading settings..." />}>
								<SettingsPage />
							</Suspense>
						</div>
					) : (
						<Suspense fallback={<ViewLoadingFallback label="Loading workspaces..." />}>
							<WorkspaceLauncher onOpenWorkspace={applyWorkspace} />
						</Suspense>
					)}
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
			{showPortForwardRestorePrompt && activeWorkspace && (
				<SavedPortForwardRestorePrompt
					workspace={activeWorkspace}
					onReview={handleOpenPortForwards}
					onDismiss={() =>
						setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id)
					}
				/>
			)}
			{viewMode === "overview" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading overview..." />}>
						<WorkspaceOverview
							workspace={activeWorkspace}
							onOpenResources={handleOpenResources}
							onOpenArgo={handleOpenArgo}
							onOpenPortForwards={handleOpenPortForwards}
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
			) : viewMode === "helm" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading Helm releases..." />}>
						<HelmPanel
							clusterContext={clusterContext}
							selectedRelease={selectedHelmRelease}
							onReleaseSelect={handleHelmReleaseSelect}
							targetRelease={targetHelmRelease}
							onTargetReleaseResolved={handleTargetHelmReleaseResolved}
						/>
					</Suspense>
				</div>
			) : viewMode === "portForwards" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading port forwards..." />}>
						<WorkspacePortForwardsPage workspace={activeWorkspace} />
					</Suspense>
				</div>
			) : viewMode === "rbac" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading RBAC inspection..." />}>
						<RbacPanel
							clusterContext={clusterContext}
							selectedNamespaces={selectedNamespaces}
							selectedView={
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
								initialSearch={resourceInitialSearch}
								onArgoAppFilterChange={setSelectedArgoAppFilter}
								onNamespacesChange={handleResourceNamespacesChange}
								onKindsChange={handleResourceKindsChange}
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
		viewMode === "helm" && selectedHelmRelease ? (
			<Suspense fallback={<ViewLoadingFallback label="Loading Helm details..." />}>
				<HelmDetailPanel
					release={selectedHelmRelease}
					onClose={handleHelmClose}
					onOpenResources={handleOpenHelmResources}
				/>
			</Suspense>
		) : viewMode === "argo" && selectedArgoApp ? (
			<Suspense fallback={<ViewLoadingFallback label="Loading app details..." />}>
				<ArgoDetailPanel app={selectedArgoApp} onClose={handleArgoClose} />
			</Suspense>
		) : selectedResource ? (
			<Suspense fallback={<ViewLoadingFallback label="Loading resource details..." />}>
				<ResourceDetailPanel
					key={selectedResourceKey}
					resource={selectedResource}
					onClose={resetResource}
					onOpenHelmRelease={handleOpenHelmReleaseFromResource}
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
				onOpenSettings={handleOpenSettings}
				onOpenPortForwards={handleOpenPortForwards}
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

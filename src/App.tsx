import "./App.css";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useDashboardState, type DashboardViewMode } from "./lib/hooks";
import { SidebarTree } from "./components/SidebarTree";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { AppMainContent } from "./app/AppMainContent";
import { AppTopBar } from "./app/AppTopBar";
import { AppDetailPanel } from "./app/AppDetailPanel";
import { AppUsageFooter } from "./app/AppUsageFooter";
import { DetailPanelFrame } from "./app/DetailPanelFrame";
import { LauncherShell } from "./app/LauncherShell";
import { useArgoDetection } from "./app/useArgoDetection";
import { useAppNavigation } from "./app/useAppNavigation";
import { useAppUpdateLaunchCheck } from "./features/app-updates/useAppUpdateLaunchCheck";
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
import { emptyStateMessage, resolveTreeScope } from "./lib/tree-nav";
import { diagnosticLog } from "./lib/diagnostics";
import {
	createTauriClient,
	getKubeconfigSources,
	stopLiveSessionsOutsideScope,
} from "./lib/tauri";
import {
	SUPPORTED_KINDS,
	type FluxResourceSummary,
	type HelmReleaseSummary,
	type ResourceKindSelection,
	type ResourceSummary,
} from "./lib/types";
import { CommandPalette } from "./features/command-palette/CommandPalette";
import { argoApplicationResourceNamespaces } from "./features/resources/helpers";
import {
	createWorkspaceScope,
	makeWorkspaceShortcuts,
	useWorkspaceStore,
	type SavedWorkspace,
	workspaceScopeContexts,
} from "./lib/workspaces";
import { queryKeys } from "./lib/queryKeys";
import {
	canQueryResourceScope,
	getAppContentTitle,
	hasAppDetailPanel,
	resourceKindLogKey,
	SIDEBAR_PROVIDER_STYLE,
} from "./app/viewHelpers";

function useLazyRef<T>(createValue: () => T): { current: T } {
	const ref = useRef<T | null>(null);
	if (ref.current === null) {
		ref.current = createValue();
	}
	return ref as { current: T };
}

function App() {
	const queryClient = useQueryClient();
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
		selectedFluxResource,
		selectedHelmRelease,
		selectedArgoAppFilter,
		resourceInitialSearch,
		resourceHealthFilter,
		viewMode,
		setClusterContext,
		setSelectedNamespaces,
		setSelectedKinds,
		select,
		setArgoDetected,
		setSelectedArgoAppFilter,
		setViewMode,
		openView,
		setResourceInitialSearch,
		selectedTreeNode,
		expandedSections,
		setSelectedTreeNode,
		toggleExpandedSection,
	} = useDashboardState();
	const activeWorkspace = workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null;
	const showUsageFooter = useSettingsState((state) => state.showUsageFooter);
	const setKubeconfigSources = useSettingsState(
		(state) => state.setKubeconfigSources,
	);
	const autoStartSavedPortForwards = useSettingsState(
		(state) => state.autoStartSavedPortForwards,
	);
	const keepLiveSessionsOnWorkspaceSwitch = useSettingsState(
		(state) => state.keepLiveSessionsOnWorkspaceSwitch,
	);
	const kubeconfigSourceKey = useSettingsState(
		(state) => state.kubeconfigSourceKey,
	);
	const [targetHelmRelease, setTargetHelmRelease] = useState<{
		name: string;
		namespace?: string;
	} | null>(null);
	const [
		dismissedPortForwardRestoreWorkspaceId,
		setDismissedPortForwardRestoreWorkspaceId,
	] = useState<string | null>(null);
	const [liveSessionCleanupMessage, setLiveSessionCleanupMessage] = useState<
		string | null
	>(null);
	const liveSessionScopeInitializedRef = useRef(false);
	const liveSessionCleanupPromiseRef = useLazyRef<Promise<void>>(() =>
		Promise.resolve(),
	);
	const autoStartedSavedPortForwardWorkspaceIdsRef = useLazyRef<Set<string>>(
		() => new Set(),
	);
	const settingsReturnViewModeRef = useRef<DashboardViewMode>("resources");
	const savedPortForwardActions = useSavedPortForwardActions(activeWorkspace);
	const appRenderCountRef = useRef(0);
	useAppUpdateLaunchCheck();

	useEffect(() => {
		const client = createTauriClient();
		void getKubeconfigSources(client).then(setKubeconfigSources).catch((error) => {
			diagnosticLog("kubeconfig.sources.load.error", {
				error: error instanceof Error ? error.message : String(error),
			});
		});
	}, [setKubeconfigSources]);

	const applyWorkspace = useCallback(
		(workspace: SavedWorkspace) => {
			setActiveWorkspace(workspace.id);
			setClusterContext(workspace.scope.clusterContext);
			setSelectedNamespaces(workspace.scope.namespaces);
			setSelectedKinds(workspace.scope.kinds);
			setDismissedPortForwardRestoreWorkspaceId(null);
			autoStartedSavedPortForwardWorkspaceIdsRef.current.delete(workspace.id);
			openView("overview", {
				treeNode: null,
				argoAppFilter:
					workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter,
			});
		},
		[
			autoStartedSavedPortForwardWorkspaceIdsRef,
			setActiveWorkspace,
			setClusterContext,
			setSelectedNamespaces,
			setSelectedKinds,
			openView,
		],
	);

	const handleClusterChange = (ctx: string) => {
		diagnosticLog("app.cluster.change", { cluster: ctx });
		setClusterContext(ctx);
		setSelectedNamespaces([]);
		setSelectedKinds(activeWorkspace?.scope.kinds ?? []);
		setArgoDetected(false);
		openView("resources", { treeNode: null });
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

	const navigation = useAppNavigation({
		activeWorkspace,
		viewMode,
		settingsReturnViewModeRef,
		setActiveWorkspace,
		setSelectedNamespaces,
		setSelectedKinds,
		setSelectedArgoAppFilter,
		setSelectedTreeNode,
		setViewMode,
		openView,
		setResourceInitialSearch,
		setDismissedPortForwardRestoreWorkspaceId,
	});
	const {
		handleOpenResources,
		handleOpenArgo,
		handleOpenIncidents,
		handleOpenLauncher,
		handleOpenSettings,
		handleBackFromSettings,
		handleOpenPortForwards,
		handleTreeNodeSelect,
	} = navigation;

	const handlePaletteResourceSelect = (resource: ResourceSummary) => {
		handleOpenResources(resource.namespace ?? undefined);
		select({ type: "resource", resource });
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
		select({ type: "argo", app });
	};

	const handleOpenArgoApplicationResources = (
		app: NonNullable<ReturnType<typeof useDashboardState>["selectedArgoApp"]>,
	) => {
		if ("status" in app) return;
		diagnosticLog("app.argo.openResources", {
			name: app.name,
			namespace: app.namespace ?? "",
		});
		setSelectedNamespaces(argoApplicationResourceNamespaces(app));
		setSelectedKinds([...SUPPORTED_KINDS]);
		select({ type: "argo", app });
		openView("resources", { treeNode: null, preserveSelection: true });
	};

	const handleArgoClose = () => {
		diagnosticLog("app.argo.close");
		select(null);
	};

	const handleFluxResourceSelect = (resource: FluxResourceSummary) => {
		diagnosticLog("app.flux.select", {
			name: resource.name,
			namespace: resource.namespace ?? "",
			kind: resource.resourceKind.kind,
		});
		select({ type: "flux", resource });
	};

	const handleFluxClose = () => {
		diagnosticLog("app.flux.close");
		select(null);
	};

	const handleHelmReleaseSelect = (release: HelmReleaseSummary) => {
		diagnosticLog("app.helm.select", {
			name: release.name,
			namespace: release.namespace,
		});
		select({ type: "helm", release });
	};

	const handleHelmClose = () => {
		diagnosticLog("app.helm.close");
		select(null);
	};

	const handleOpenHelmResources = (release: HelmReleaseSummary) => {
		diagnosticLog("app.helm.openResources", {
			name: release.name,
			namespace: release.namespace,
		});
		setSelectedNamespaces([release.namespace]);
		setSelectedKinds([...SUPPORTED_KINDS]);
		openView("resources", {
			treeNode: null,
			initialSearch: release.name,
		});
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
		openView("helm");
	};

	const handleTargetHelmReleaseResolved = useCallback(() => {
		setTargetHelmRelease(null);
	}, [setTargetHelmRelease]);

	const handleResourceNamespacesChange = useCallback(
		(namespaces: string[]) => {
			setSelectedTreeNode(null);
			select(null);
			setSelectedNamespaces(namespaces);
		},
		[
			setSelectedTreeNode,
			select,
			setSelectedNamespaces,
		],
	);

	const handleResourceKindsChange = useCallback(
		(kinds: ResourceKindSelection[]) => {
			setSelectedTreeNode(null);
			select(null);
			setSelectedKinds(kinds);
		},
		[
			setSelectedTreeNode,
			select,
			setSelectedKinds,
		],
	);

	const handleGitOpsFilterChange = useCallback(
		(filter: string) => {
			setSelectedArgoAppFilter(filter);
			if (!activeWorkspace) return;
			const legacyArgoFilter = filter.includes(":") ? "" : filter;
			updateWorkspace(activeWorkspace.id, {
				scope: {
					...activeWorkspace.scope,
					gitOpsFilter: filter,
					argoAppFilter: legacyArgoFilter,
				},
			});
		},
		[activeWorkspace, setSelectedArgoAppFilter, updateWorkspace],
	);

	const handleResourceSelect = (resource: ResourceSummary) => {
		select({ type: "resource", resource });
	};

	useArgoDetection(clusterContext, setArgoDetected);

	const liveSessionAllowedContexts = useMemo(
		() => (activeWorkspace ? workspaceScopeContexts(activeWorkspace.scope) : []),
		[activeWorkspace],
	);
	const liveSessionAllowedContextsKey = liveSessionAllowedContexts.join("|");

	useEffect(() => {
		if (!liveSessionScopeInitializedRef.current) {
			liveSessionScopeInitializedRef.current = true;
			return;
		}
		if (keepLiveSessionsOnWorkspaceSwitch) {
			liveSessionCleanupPromiseRef.current = Promise.resolve();
			return;
		}

		const client = createTauriClient();
		const cleanup = (async () => {
			try {
				const result = await stopLiveSessionsOutsideScope(client, {
					allowedClusterContexts: liveSessionAllowedContexts,
					kubeconfigSourceKey,
				});
				await Promise.all([
					queryClient.invalidateQueries({ queryKey: queryKeys.portForwards() }),
					queryClient.invalidateQueries({ queryKey: queryKeys.podExecSessions() }),
				]);
				const stopped =
					result.stoppedPortForwards + result.stoppedPodExecSessions;
				if (stopped > 0) {
					setLiveSessionCleanupMessage(
						`Stopped ${result.stoppedPortForwards} port ${result.stoppedPortForwards === 1 ? "forward" : "forwards"} and ${result.stoppedPodExecSessions} exec ${result.stoppedPodExecSessions === 1 ? "session" : "sessions"} outside this workspace.`,
					);
				}
			} catch (error) {
				setLiveSessionCleanupMessage(
					`Live session cleanup failed: ${error instanceof Error ? error.message : String(error)}`,
				);
			}
		})();
		liveSessionCleanupPromiseRef.current = cleanup;
	}, [
		keepLiveSessionsOnWorkspaceSwitch,
		kubeconfigSourceKey,
		liveSessionAllowedContexts,
		liveSessionAllowedContextsKey,
		liveSessionCleanupPromiseRef,
		queryClient,
	]);

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
		void (async () => {
			await liveSessionCleanupPromiseRef.current;
			await savedPortForwardActions.startAll();
		})();
	}, [
		activeWorkspace,
		autoStartSavedPortForwards,
		autoStartedSavedPortForwardWorkspaceIdsRef,
		liveSessionCleanupPromiseRef,
		savedPortForwardActions,
	]);

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
		appRenderCountRef.current += 1;
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
			<ErrorBoundary label="launcher">
				<LauncherShell
					clusterContext={clusterContext}
					viewMode={viewMode}
					showUsageFooter={showUsageFooter}
					onClusterChange={handleClusterChange}
					onOpenLauncher={handleOpenLauncher}
					onOpenSettings={handleOpenSettings}
					onBackFromSettings={handleBackFromSettings}
					onOpenWorkspace={applyWorkspace}
				/>
			</ErrorBoundary>
		);
	}

	const mainContent = (
		<ErrorBoundary label="main content">
			<AppMainContent
				activeWorkspace={activeWorkspace}
				viewMode={viewMode}
				liveSessionCleanupMessage={liveSessionCleanupMessage}
				onDismissLiveSessionCleanup={() => setLiveSessionCleanupMessage(null)}
				showPortForwardRestorePrompt={showPortForwardRestorePrompt}
				onDismissPortForwardRestore={() =>
					setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id)
				}
				onReviewPortForwards={handleOpenPortForwards}
				onOpenResources={handleOpenResources}
				onOpenArgo={handleOpenArgo}
				onOpenIncidents={handleOpenIncidents}
				onOpenPortForwards={handleOpenPortForwards}
				onOpenLauncher={handleOpenLauncher}
				onBackFromSettings={handleBackFromSettings}
				clusterContext={clusterContext}
				selectedArgoApp={selectedArgoApp}
				onArgoItemSelect={handleArgoAppSelect}
				onOpenArgoApplicationResources={handleOpenArgoApplicationResources}
				selectedFluxResource={selectedFluxResource}
				onFluxResourceSelect={handleFluxResourceSelect}
				selectedTreeNode={selectedTreeNode}
				selectedHelmRelease={selectedHelmRelease}
				onHelmReleaseSelect={handleHelmReleaseSelect}
				targetHelmRelease={targetHelmRelease}
				onTargetHelmReleaseResolved={handleTargetHelmReleaseResolved}
				selectedNamespaces={selectedNamespaces}
				canQueryResources={canQueryResources}
				computedNamespaces={computedNamespaces}
				computedKinds={computedKinds}
				selectedArgoAppFilter={selectedArgoAppFilter}
				selectedResource={selectedResource}
				resourceHealthFilter={resourceHealthFilter}
				resourceInitialSearch={resourceInitialSearch}
				onArgoAppFilterChange={handleGitOpsFilterChange}
				onNamespacesChange={handleResourceNamespacesChange}
				onKindsChange={handleResourceKindsChange}
				onResourceSelect={handleResourceSelect}
				emptyMsg={emptyMsg}
			/>
		</ErrorBoundary>
	);

	const detailPanel = hasAppDetailPanel(viewMode, selectedHelmRelease !== null, selectedArgoApp !== null || selectedFluxResource !== null, selectedResource !== null) ? (
		<ErrorBoundary label="detail panel">
			<AppDetailPanel
				viewMode={viewMode}
				selectedHelmRelease={selectedHelmRelease}
				selectedArgoApp={selectedArgoApp}
				selectedFluxResource={selectedFluxResource}
				selectedResource={selectedResource}
				selectedResourceKey={selectedResourceKey}
				onHelmClose={handleHelmClose}
				onArgoClose={handleArgoClose}
				onFluxClose={handleFluxClose}
				onResourceClose={() => select(null)}
				onOpenHelmResources={handleOpenHelmResources}
				onOpenHelmReleaseFromResource={handleOpenHelmReleaseFromResource}
			/>
		</ErrorBoundary>
	) : null;

	return (
		<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<CommandPalette
				clusterContext={clusterContext}
				kubeconfigEnvVar={kubeconfigSourceKey}
				workspace={activeWorkspace}
				navigation={navigation}
				onSelectResource={handlePaletteResourceSelect}
			/>
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

import "./App.css";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDashboardState } from "./lib/hooks";
import { ClusterSelector } from "./components/ClusterSelector";
import { SidebarTree } from "./components/SidebarTree";
import { ResourceList } from "./components/ResourceList";
import { ResourceDetailPanel } from "./features/resource-detail/ResourceDetailPanel";
import { ArgoCDPanel } from "./features/argo/ArgoCDPanel";
import { ArgoDetailPanel } from "./features/argo/ArgoDetailPanel";
import { Search, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SettingsPage } from "./features/settings/SettingsPage";

import { createTauriClient, detectArgoCD } from "./lib/tauri";
import {
	resolveTreeScope,
	emptyStateMessage,
	type TreeNodeId,
} from "./lib/tree-nav";
import { diagnosticLog } from "./lib/diagnostics";

const DETAIL_PANEL_DEFAULT_WIDTH = 480;
const DETAIL_PANEL_MIN_WIDTH = 390;
const MAIN_PANEL_MIN_WIDTH = 360;
const SIDEBAR_WIDTH = 260;

function clampDetailPanelWidth(width: number): number {
	const viewportWidth =
		typeof window === "undefined" ? 1440 : window.innerWidth;
	const maxWidth = Math.max(
		DETAIL_PANEL_MIN_WIDTH,
		viewportWidth - SIDEBAR_WIDTH - MAIN_PANEL_MIN_WIDTH,
	);
	return Math.min(Math.max(width, DETAIL_PANEL_MIN_WIDTH), maxWidth);
}

function App() {
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
	const [detailPanelWidth, setDetailPanelWidth] = useState(
		DETAIL_PANEL_DEFAULT_WIDTH,
	);
	const appRenderCountRef = useRef(0);
	appRenderCountRef.current += 1;

	const handleClusterChange = (ctx: string) => {
		diagnosticLog("app.cluster.change", { cluster: ctx });
		setClusterContext(ctx);
		// Clear inspector state on context switch
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedArgoAppFilter("");
		setSelectedNamespaces([]);
		setArgoDetected(false);
		setSelectedTreeNode(null);
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
		} else if (viewMode === "argo" || viewMode === "settings") {
			// Leaving Argo → switch to resources, clear Argo state
			setViewMode("resources");
			setSelectedArgoApp(null);
		}

		setSelectedTreeNode(nodeId);
	};

	const selectedResourceKey = useMemo(
		() =>
			selectedResource
				? `${selectedResource.cluster}::${selectedResource.kind}::${selectedResource.namespace ?? ""}::${selectedResource.name}`
				: null,
		[
			selectedResource?.cluster,
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

	const handleDetailResizeStart = useCallback(
		(event: React.PointerEvent<HTMLDivElement>) => {
			event.preventDefault();
			const startX = event.clientX;
			const startWidth = detailPanelWidth;

			const handlePointerMove = (moveEvent: PointerEvent) => {
				const nextWidth = startWidth + startX - moveEvent.clientX;
				setDetailPanelWidth(clampDetailPanelWidth(nextWidth));
			};

			const handlePointerUp = () => {
				document.body.style.cursor = "";
				document.body.style.userSelect = "";
				window.removeEventListener("pointermove", handlePointerMove);
				window.removeEventListener("pointerup", handlePointerUp);
			};

			document.body.style.cursor = "col-resize";
			document.body.style.userSelect = "none";
			window.addEventListener("pointermove", handlePointerMove);
			window.addEventListener("pointerup", handlePointerUp, { once: true });
		},
		[detailPanelWidth],
	);

	// Detect Argo CD when cluster context changes
	useEffect(() => {
		if (!clusterContext) {
			setArgoDetected(false);
			return;
		}
		let cancelled = false;
		const client = createTauriClient();
		detectArgoCD(client, clusterContext)
			.then((detected) => {
				if (!cancelled) {
					diagnosticLog("app.argo.detect.done", {
						cluster: clusterContext,
						detected,
					});
					setArgoDetected(detected);
				}
			})
			.catch(() => {
				if (!cancelled) {
					diagnosticLog("app.argo.detect.error", { cluster: clusterContext });
					setArgoDetected(false);
				}
			});
		return () => {
			cancelled = true;
		};
	}, [clusterContext, setArgoDetected]);

	// Compute scope from selected tree node
	const scope = useMemo(
		() => resolveTreeScope(selectedTreeNode),
		[selectedTreeNode],
	);

	// Derive selectedKinds and selectedNamespaces from tree selection
	const computedKinds = useMemo<string[]>(() => {
		if (scope.kinds.length > 0) return scope.kinds;
		// Fall back to hook state for kind toggles (backwards compat)
		return selectedKinds as string[];
	}, [scope.kinds, selectedKinds]);

	const computedNamespaces = useMemo<string[]>(() => {
		if (scope.namespace) return [scope.namespace];
		return selectedNamespaces;
	}, [scope.namespace, selectedNamespaces]);

	// SECTIONS import for content title
	const SECTIONS = useMemo(
		() => ({
			clusterOverview: { label: "Cluster Overview" },
			namespaces: { label: "Namespaces" },
			workloads: { label: "Workloads" },
			network: { label: "Network" },
			config: { label: "Config" },
			storage: { label: "Storage" },
			discovered: { label: "Discovered" },
			argo: { label: "Argo CD" },
		}),
		[],
	);

	// Determine content title from scope
	const contentTitle = useMemo(() => {
		if (viewMode === "settings") return "Settings";
		if (viewMode === "argo") {
			if (selectedTreeNode?.type === "kind" && selectedTreeNode.kind) {
				return `${selectedTreeNode.kind}`;
			}
			return "Argo CD";
		}
		if (!scope.section) return "Kubernetes Resources";
		if (scope.section === "clusterOverview") {
			if (scope.kinds.length === 1) return `${scope.kinds[0]} Resources`;
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
		if (scope.kinds.length === 1) return `${scope.kinds[0]} Resources`;
		if (scope.kinds.length > 1)
			return SECTIONS[scope.section]?.label ?? scope.section;
		return (
			SECTIONS[scope.section as keyof typeof SECTIONS]?.label ?? scope.section
		);
	}, [scope, viewMode, selectedTreeNode]);

	const canQueryResources =
		computedKinds.length > 0 &&
		!!clusterContext &&
		(scope.clusterScoped || computedNamespaces.length > 0);

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
			kinds: computedKinds.join("|"),
			namespaces: computedNamespaces.join("|"),
			selectedResource: selectedResourceKey ?? "",
			argoFilter: selectedArgoAppFilter,
		});
	});

	const mainContent = (
		<main className="flex h-full w-full min-w-0 flex-col overflow-hidden">
			{viewMode === "settings" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<SettingsPage />
				</div>
			) : viewMode === "argo" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
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
				</div>
			) : (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					{canQueryResources ? (
						<ResourceList
							clusterContext={clusterContext}
							selectedNamespaces={computedNamespaces}
							selectedKinds={computedKinds}
							selectedArgoAppFilter={selectedArgoAppFilter}
							onArgoAppFilterChange={setSelectedArgoAppFilter}
							onResourceSelect={setSelectedResource}
						/>
					) : (
						<div className="p-8 text-center text-sm text-muted-foreground">
							{emptyMsg}
						</div>
					)}
				</div>
			)}
		</main>
	);

	const detailPanel =
		viewMode === "argo" && selectedArgoApp ? (
			<ArgoDetailPanel app={selectedArgoApp} onClose={handleArgoClose} />
		) : selectedResource ? (
			<ResourceDetailPanel
				key={selectedResourceKey}
				resource={selectedResource}
				onClose={resetResource}
			/>
		) : null;

	return (
		<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			{/* Top Bar */}
			<header className="flex h-12 shrink-0 items-center gap-4 border-b bg-sidebar px-4 [-webkit-app-region:drag]">
				<div className="flex shrink-0 items-center gap-3 [-webkit-app-region:no-drag]">
					<ClusterSelector onClusterChange={handleClusterChange} />
				</div>
				<div className="flex min-w-0 flex-1 items-center justify-center">
					<span className="truncate whitespace-nowrap text-sm font-semibold">
						{contentTitle}
					</span>
				</div>
				<div className="flex shrink-0 items-center">
					<Button
						type="button"
						variant="ghost"
						size="icon"
						className="mr-2 size-8 text-muted-foreground [-webkit-app-region:no-drag]"
						aria-label="Open settings"
						onClick={() => {
							setViewMode("settings");
							setSelectedResource(null);
							setSelectedArgoApp(null);
						}}
					>
						<Settings />
					</Button>
					<div className="flex cursor-pointer items-center gap-2 whitespace-nowrap rounded-md border bg-background/50 px-3 py-1.5 text-xs text-muted-foreground transition-colors hover:border-ring hover:text-foreground">
						<Search className="size-3.5" aria-hidden="true" />
						<span>Search resources…</span>
					</div>
				</div>
			</header>

			{/* Main row: sidebar + content + inspector */}
			<div className="flex min-h-0 flex-1 flex-row overflow-hidden">
				{/* Left Sidebar Tree */}
				<aside className="flex w-[260px] min-w-[260px] shrink-0 flex-col overflow-y-auto overflow-x-hidden border-r bg-sidebar">
					<SidebarTree
						clusterContext={clusterContext}
						selectedNode={selectedTreeNode}
						expandedSections={expandedSections}
						onNodeSelect={handleTreeNodeSelect}
						onSectionToggle={toggleExpandedSection}
					/>
				</aside>

				{detailPanel ? (
					<div className="flex min-w-0 flex-1 overflow-hidden">
						<div className="min-w-0 flex-1 overflow-hidden">
							{mainContent}
						</div>
						<div
							role="separator"
							aria-orientation="vertical"
							aria-label="Resize details panel"
							className="group relative flex w-2 shrink-0 cursor-col-resize items-center justify-center"
							onPointerDown={handleDetailResizeStart}
							onDoubleClick={() =>
								setDetailPanelWidth(DETAIL_PANEL_DEFAULT_WIDTH)
							}
						>
							<div className="h-full w-px bg-border transition-colors group-hover:bg-ring" />
							<div className="absolute h-8 w-1 rounded-full bg-border transition-colors group-hover:bg-ring" />
						</div>
						<div
							className="h-full shrink-0 overflow-hidden"
							style={{
								width: clampDetailPanelWidth(detailPanelWidth),
								minWidth: DETAIL_PANEL_MIN_WIDTH,
							}}
						>
							{detailPanel}
						</div>
					</div>
				) : (
					mainContent
				)}
			</div>
		</div>
	);
}

export default App;

import "./App.css";
import { useEffect, useMemo, useRef } from "react";
import { useDashboardState } from "./lib/hooks";
import { ClusterSelector } from "./components/ClusterSelector";
import { SidebarTree } from "./components/SidebarTree";
import { ResourceList } from "./components/ResourceList";
import { ResourceDetailPanel } from "./features/resource-detail/ResourceDetailPanel";
import { ArgoCDPanel } from "./features/argo/ArgoCDPanel";
import { ArgoDetailPanel } from "./features/argo/ArgoDetailPanel";

import { createTauriClient, detectArgoCD } from "./lib/tauri";
import {
	resolveTreeScope,
	emptyStateMessage,
	type TreeNodeId,
} from "./lib/tree-nav";
import { diagnosticLog } from "./lib/diagnostics";

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
		} else if (viewMode === "argo") {
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
			argo: { label: "Argo CD" },
		}),
		[],
	);

	// Determine content title from scope
	const contentTitle = useMemo(() => {
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

	return (
		<div className="app-shell">
			{/* Top Bar */}
			<header className="top-bar">
				<div className="top-bar-left">
					<ClusterSelector onClusterChange={handleClusterChange} />
				</div>
				<div className="top-bar-center">
					<span className="top-bar-title">{contentTitle}</span>
				</div>
				<div className="top-bar-right">
					<div className="global-search-placeholder">
						<svg
							width="14"
							height="14"
							viewBox="0 0 14 14"
							fill="#888"
							aria-hidden="true"
						>
							<path
								d="M10 6a4 4 0 11-2.77 1.21l-2.96 2.96a.5.5 0 01-.35.15.5.5 0 01-.5-.5.5.5 0 01.15-.35l2.96-2.96A4 4 0 0110 6zm-5 4a3 3 0 100-6 3 3 0 000 6z"
								fillRule="evenodd"
							/>
						</svg>
						<span>Search resources…</span>
					</div>
				</div>
			</header>

			{/* Main row: sidebar + content + inspector */}
			<div className="app-body">
				{/* Left Sidebar Tree */}
				<aside className="sidebar">
					<SidebarTree
						clusterContext={clusterContext}
						selectedNode={selectedTreeNode}
						expandedSections={expandedSections}
						onNodeSelect={handleTreeNodeSelect}
						onSectionToggle={toggleExpandedSection}
					/>
				</aside>

				{/* Main Content */}
				<main className="main-content">
					{viewMode === "argo" ? (
						<>
							<div className="resource-area">
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
						</>
					) : (
						<>
							<div className="resource-area">
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
									<div className="resource-area-empty">{emptyMsg}</div>
								)}
							</div>
						</>
					)}
				</main>

				{/* Right Detail Panel */}
				{viewMode === "argo" && selectedArgoApp ? (
					<ArgoDetailPanel app={selectedArgoApp} onClose={handleArgoClose} />
				) : selectedResource ? (
					<ResourceDetailPanel
						key={selectedResourceKey}
						resource={selectedResource}
						onClose={resetResource}
					/>
				) : null}
			</div>
		</div>
	);
}

export default App;

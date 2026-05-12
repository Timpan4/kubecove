import "./App.css";
import { useEffect, useMemo } from "react";
import { useDashboardState } from "./lib/hooks";
import { ClusterSelector } from "./components/ClusterSelector";
import { SidebarTree } from "./components/SidebarTree";
import { ResourceList } from "./components/ResourceList";
import { ResourceDetailPanel } from "./features/resource-detail/ResourceDetailPanel";
import { ArgoCDPanel } from "./features/argo/ArgoCDPanel";
import { ArgoDetailPanel } from "./features/argo/ArgoDetailPanel";
import type { ResourceSummary, ArgoApplicationSummary } from "./lib/types";
import { CLUSTER_SCOPED_KINDS } from "./lib/types";
import { createTauriClient, detectArgoCD } from "./lib/tauri";
import {
  resolveTreeScope,
  emptyStateMessage,
  type TreeNodeId,
} from "./lib/tree-nav";

function isClusterScopedOnly(kinds: string[]): boolean {
  return kinds.length > 0 && kinds.every((k) => (CLUSTER_SCOPED_KINDS as readonly string[]).includes(k));
}

function App() {
  const {
    clusterContext,
    selectedNamespaces,
    selectedKinds,
    selectedResource,
    argoDetected,
    selectedArgoApp,
    viewMode,
    setClusterContext,
    setSelectedNamespaces,
    setSelectedResource,
    setArgoDetected,
    setSelectedArgoApp,
    setViewMode,
    selectedTreeNode,
    expandedSections,
    setSelectedTreeNode,
    toggleExpandedSection,
  } = useDashboardState();

  const handleClusterChange = (ctx: string) => {
    setClusterContext(ctx);
    // Clear inspector state on context switch
    setSelectedResource(null);
    setSelectedArgoApp(null);
    setSelectedNamespaces([]);
    setArgoDetected(false);
    setSelectedTreeNode(null);
    setViewMode("resources");
  };

  const handleTreeNodeSelect = (nodeId: TreeNodeId) => {
    const scope = resolveTreeScope(nodeId);

    // Argo section or child → switch to argo view, clear resource state
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

  const handleResourceSelect = (resource: ResourceSummary) => {
    setSelectedResource(resource);
  };

  const handleArgoAppSelect = (app: ArgoApplicationSummary) => {
    setSelectedArgoApp(app);
  };

  const handleArgoClose = () => {
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
        if (!cancelled) setArgoDetected(detected);
      })
      .catch(() => {
        if (!cancelled) setArgoDetected(false);
      });
    return () => { cancelled = true; };
  }, [clusterContext, setArgoDetected]);

  // Compute scope from selected tree node
  const scope = useMemo(() => resolveTreeScope(selectedTreeNode), [selectedTreeNode]);

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
  const SECTIONS = useMemo(() => ({
    clusterOverview: { label: "Cluster Overview" },
    namespaces: { label: "Namespaces" },
    workloads: { label: "Workloads" },
    network: { label: "Network" },
    config: { label: "Config" },
    storage: { label: "Storage" },
    argo: { label: "Argo CD" },
  }), []);

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
    if (scope.kinds.length > 1) return SECTIONS[scope.section]?.label ?? scope.section;
    return SECTIONS[scope.section as keyof typeof SECTIONS]?.label ?? scope.section;
  }, [scope, viewMode, selectedTreeNode]);

  const emptyMsg = useMemo(
    () => emptyStateMessage(scope, !!clusterContext),
    [scope, clusterContext]
  );

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
            <svg width="14" height="14" viewBox="0 0 14 14" fill="#888" aria-hidden="true">
              <path d="M10 6a4 4 0 11-2.77 1.21l-2.96 2.96a.5.5 0 01-.35.15.5.5 0 01-.5-.5.5.5 0 01.15-.35l2.96-2.96A4 4 0 0110 6zm-5 4a3 3 0 100-6 3 3 0 000 6z" fillRule="evenodd" />
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
          {viewMode === "argo" && argoDetected ? (
            <>
              <div className="resource-area">
                <ArgoCDPanel
                  clusterContext={clusterContext}
                  selectedArgoApp={selectedArgoApp}
                  onAppSelect={handleArgoAppSelect}
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
                {computedKinds.length > 0 &&
                (computedNamespaces.length > 0 || isClusterScopedOnly(computedKinds)) &&
                clusterContext ? (
                  <ResourceList
                    clusterContext={clusterContext}
                    selectedNamespaces={computedNamespaces}
                    selectedKinds={computedKinds}
                    selectedResource={selectedResource}
                    onResourceSelect={handleResourceSelect}
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
            resource={selectedResource}
            onClose={() => setSelectedResource(null)}
          />
        ) : null}
      </div>
    </div>
  );
}

export default App;
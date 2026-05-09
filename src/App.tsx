import "./App.css";
import { useDashboardState } from "./lib/hooks";
import { ClusterSelector } from "./components/ClusterSelector";
import { NamespaceList } from "./components/NamespaceList";
import { KindList } from "./components/KindList";
import { ResourceList } from "./components/ResourceList";
import { ResourceDetailPanel } from "./features/resource-detail/ResourceDetailPanel";
import type { ResourceSummary } from "./lib/types";

function App() {
  const {
    clusterContext,
    selectedNamespaces,
    selectedKinds,
    selectedResource,
    setClusterContext,
    setSelectedNamespaces,
    toggleKind,
    setSelectedResource,
  } = useDashboardState();

  const handleClusterChange = (ctx: string) => {
    setClusterContext(ctx);
    setSelectedResource(null);
    setSelectedNamespaces([]);
  };

  const handleNamespaceChange = (namespaces: string[]) => {
    setSelectedNamespaces(namespaces);
  };

  const handleResourceSelect = (resource: ResourceSummary) => {
    setSelectedResource(resource);
  };

  return (
    <div className="app-shell">
      {/* Left Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-section cluster-section">
          <ClusterSelector onClusterChange={handleClusterChange} />
        </div>

        <div className="sidebar-section namespace-section">
          <NamespaceList
            clusterContext={clusterContext}
            selectedNamespaces={selectedNamespaces}
            onNamespaceChange={handleNamespaceChange}
          />
        </div>

        <div className="sidebar-section kind-section">
          <KindList selectedKinds={selectedKinds} onToggleKind={toggleKind} />
        </div>
      </aside>

      {/* Main Content */}
      <main className="main-content">
        <header className="content-header">
          <h2 className="content-title">
            {selectedKinds.length > 0
              ? `${selectedKinds.join(", ")} Resources`
              : "Select a resource kind"}
          </h2>
        </header>

        <div className="resource-area">
          {selectedKinds.length > 0 && selectedNamespaces.length > 0 && clusterContext ? (
            <ResourceList
              clusterContext={clusterContext}
              selectedNamespaces={selectedNamespaces}
              selectedKinds={selectedKinds}
              selectedResource={selectedResource}
              onResourceSelect={handleResourceSelect}
            />
          ) : (
            <div className="resource-area-empty">
              {!clusterContext
                ? "Select a cluster context first"
                : selectedKinds.length === 0
                ? "Select one or more resource kinds from the sidebar"
                : "Select one or more namespaces from the sidebar"}
            </div>
          )}
        </div>
      </main>

      {/* Right Detail Panel */}
      {selectedResource && (
        <ResourceDetailPanel
          resource={selectedResource}
          onClose={() => setSelectedResource(null)}
        />
      )}
    </div>
  );
}

export default App;

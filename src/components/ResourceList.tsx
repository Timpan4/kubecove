import { useState, useEffect, useCallback } from "react";
import { createTauriClient, listResources } from "../lib/tauri";
import type { ResourceSummary } from "../lib/types";

interface ResourceListProps {
  clusterContext: string;
  kind: string;
  namespace?: string;
  onResourceSelect: (resource: ResourceSummary) => void;
}

export function ResourceList({ clusterContext, kind, namespace, onResourceSelect }: ResourceListProps) {
  const [resources, setResources] = useState<ResourceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadResources = useCallback(async () => {
    if (!clusterContext || !kind) {
      setResources([]);
      setLoading(false);
      return;
    }

    const client = createTauriClient();
    setLoading(true);
    setError(null);
    try {
      const res = await listResources(client, clusterContext, kind, namespace);
      setResources(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load resources");
    } finally {
      setLoading(false);
    }
  }, [clusterContext, kind, namespace]);

  useEffect(() => {
    loadResources();
  }, [loadResources]);

  if (!clusterContext) {
    return <div className="resource-list empty">Select a cluster context first</div>;
  }

  if (!kind) {
    return <div className="resource-list empty">Select a resource kind</div>;
  }

  if (loading) {
    return <div className="resource-list">Loading {kind}s...</div>;
  }

  if (error) {
    return (
      <div className="resource-list error">
        Error: {error}
        <button onClick={loadResources} className="retry-btn">Retry</button>
      </div>
    );
  }

  if (resources.length === 0) {
    return <div className="resource-list empty">No {kind}s found</div>;
  }

  return (
    <div className="resource-list">
      <h3 className="resource-list-title">{kind}s in {namespace || "all namespaces"}</h3>
      <table className="resource-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Namespace</th>
            <th>Cluster</th>
            <th>Age</th>
          </tr>
        </thead>
        <tbody>
          {resources.map((res) => (
            <tr
              key={`${res.kind}-${res.name}-${res.namespace}`}
              className="resource-row"
              onClick={() => onResourceSelect(res)}
            >
              <td className="resource-name">{res.name}</td>
              <td className="resource-namespace">{res.namespace || "-"}</td>
              <td className="resource-cluster">{res.cluster}</td>
              <td className="resource-age">{res.age}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
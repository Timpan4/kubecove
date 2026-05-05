import { useState, useEffect, useCallback } from "react";
import { createTauriClient, listNamespaces } from "../lib/tauri";
import type { NamespaceSummary } from "../lib/types";

interface NamespaceListProps {
  clusterContext: string;
  onNamespaceSelect: (namespace: string) => void;
}

export function NamespaceList({ clusterContext, onNamespaceSelect }: NamespaceListProps) {
  const [namespaces, setNamespaces] = useState<NamespaceSummary[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadNamespaces = useCallback(async () => {
    if (!clusterContext) {
      setNamespaces([]);
      setLoading(false);
      return;
    }

    const client = createTauriClient();
    setLoading(true);
    setError(null);
    try {
      const ns = await listNamespaces(client, clusterContext);
      setNamespaces(ns);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load namespaces");
    } finally {
      setLoading(false);
    }
  }, [clusterContext]);

  useEffect(() => {
    loadNamespaces();
  }, [loadNamespaces]);

  const handleClick = (name: string) => {
    setSelected(name);
    onNamespaceSelect(name);
  };

  if (!clusterContext) {
    return <div className="namespace-list empty">Select a cluster context first</div>;
  }

  if (loading) {
    return <div className="namespace-list">Loading namespaces...</div>;
  }

  if (error) {
    return (
      <div className="namespace-list error">
        Error: {error}
        <button onClick={loadNamespaces} className="retry-btn">Retry</button>
      </div>
    );
  }

  if (namespaces.length === 0) {
    return <div className="namespace-list empty">No namespaces found</div>;
  }

  return (
    <div className="namespace-list">
      <h3 className="namespace-list-title">Namespaces</h3>
      <ul className="namespace-items">
        {namespaces.map((ns) => (
          <li
            key={ns.name}
            className={`namespace-item ${selected === ns.name ? "selected" : ""}`}
            onClick={() => handleClick(ns.name)}
          >
            <span className="namespace-name">{ns.name}</span>
            <span className="namespace-age">{ns.age}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
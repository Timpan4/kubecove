import { useState, useEffect, useCallback } from "react";
import { createTauriClient, listNamespaces } from "../lib/tauri";
import type { NamespaceSummary } from "../lib/types";

interface NamespaceListProps {
  clusterContext: string;
  selectedNamespaces: string[];
  onNamespaceChange: (namespaces: string[]) => void;
}

export function NamespaceList({ clusterContext, selectedNamespaces, onNamespaceChange }: NamespaceListProps) {
  const [namespaces, setNamespaces] = useState<NamespaceSummary[]>([]);
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

  const handleToggleAll = () => {
    if (selectedNamespaces.length === namespaces.length) {
      onNamespaceChange([]);
    } else {
      onNamespaceChange(namespaces.map((ns) => ns.name));
    }
  };

  const handleToggleOne = (name: string) => {
    if (selectedNamespaces.includes(name)) {
      onNamespaceChange(selectedNamespaces.filter((n) => n !== name));
    } else {
      onNamespaceChange([...selectedNamespaces, name]);
    }
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

  const allSelected = namespaces.length > 0 && selectedNamespaces.length === namespaces.length;
  const someSelected = selectedNamespaces.length > 0 && !allSelected;

  return (
    <div className="namespace-list">
      <div className="namespace-list-header">
        <h3 className="namespace-list-title">Namespaces</h3>
        <button
          className="select-all-btn"
          onClick={handleToggleAll}
          type="button"
        >
          {allSelected ? "Deselect All" : someSelected ? "Deselect All" : "Select All"}
        </button>
      </div>
      <ul className="namespace-items">
        {namespaces.map((ns) => (
          <li
            key={ns.name}
            className={`namespace-item ${selectedNamespaces.includes(ns.name) ? "selected" : ""}`}
          >
            <label className="namespace-checkbox-label">
              <input
                type="checkbox"
                checked={selectedNamespaces.includes(ns.name)}
                onChange={() => handleToggleOne(ns.name)}
                className="namespace-checkbox"
              />
              <span className="namespace-name">{ns.name}</span>
              <span className="namespace-age">{ns.age}</span>
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}
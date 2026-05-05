import { useState, useEffect, useCallback } from "react";
import { createTauriClient, listKubeContexts } from "../lib/tauri";
import type { ClusterContext } from "../lib/types";

interface ClusterSelectorProps {
  onClusterChange: (cluster: string) => void;
}

export function ClusterSelector({ onClusterChange }: ClusterSelectorProps) {
  const [clusters, setClusters] = useState<ClusterContext[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadClusters = useCallback(async () => {
    const client = createTauriClient();
    setLoading(true);
    setError(null);
    try {
      const contexts = await listKubeContexts(client);
      setClusters(contexts);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load contexts");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadClusters();
  }, [loadClusters]);

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    setSelected(value);
    onClusterChange(value);
  };

  if (loading) {
    return <div className="cluster-selector">Loading contexts...</div>;
  }

  if (error) {
    return (
      <div className="cluster-selector error">
        Error: {error}
        <button onClick={loadClusters} className="retry-btn">Retry</button>
      </div>
    );
  }

  if (clusters.length === 0) {
    return <div className="cluster-selector empty">No contexts found</div>;
  }

  return (
    <div className="cluster-selector">
      <label htmlFor="cluster-select">Cluster Context:</label>
      <select
        id="cluster-select"
        value={selected}
        onChange={handleChange}
        className="cluster-select"
      >
        <option value="">Select a context...</option>
        {clusters.map((ctx) => (
          <option key={ctx.name} value={ctx.name}>
            {ctx.name}
          </option>
        ))}
      </select>
    </div>
  );
}
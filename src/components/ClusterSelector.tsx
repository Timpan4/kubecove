import { useState, useEffect, useCallback, useRef } from "react";
import { createTauriClient, listKubeContexts } from "../lib/tauri";
import type { ClusterContext } from "../lib/types";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ClusterSelectorProps {
  onClusterChange: (cluster: string) => void;
}

export function ClusterSelector({ onClusterChange }: ClusterSelectorProps) {
  const [clusters, setClusters] = useState<ClusterContext[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const selectedRef = useRef(selected);
  const onClusterChangeRef = useRef(onClusterChange);

  useEffect(() => {
    selectedRef.current = selected;
  }, [selected]);

  useEffect(() => {
    onClusterChangeRef.current = onClusterChange;
  }, [onClusterChange]);

  const loadClusters = useCallback(async () => {
    const client = createTauriClient();
    setLoading(true);
    setError(null);
    try {
      const contexts = await listKubeContexts(client);
      setClusters(contexts);
      const currentContext = contexts.find((ctx) => ctx.isCurrent);
      const preferredContext = currentContext ?? contexts[0];
      const selectedContext = selectedRef.current;
      const selectedStillExists = contexts.some((ctx) => ctx.name === selectedContext);
      if ((!selectedContext || !selectedStillExists) && preferredContext) {
        selectedRef.current = preferredContext.name;
        setSelected(preferredContext.name);
        onClusterChangeRef.current(preferredContext.name);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load contexts");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadClusters();
  }, [loadClusters]);

  const handleChange = (value: string) => {
    selectedRef.current = value;
    setSelected(value);
    onClusterChange(value);
  };

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        Loading contexts...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center gap-2 text-xs text-destructive">
        Error: {error}
        <Button type="button" variant="outline" size="sm" onClick={loadClusters}>
          Retry
        </Button>
      </div>
    );
  }

  if (clusters.length === 0) {
    return <div className="text-xs text-muted-foreground">No contexts found</div>;
  }

  return (
    <div className="flex flex-row items-center gap-2">
      <span
        id="cluster-select-label"
        className="whitespace-nowrap text-[0.6875rem] font-bold uppercase tracking-wide text-muted-foreground"
      >
        Cluster Context:
      </span>
      <Select
        value={selected}
        onValueChange={handleChange}
      >
        <SelectTrigger
          id="cluster-select"
          aria-labelledby="cluster-select-label"
          className="h-8 min-w-40 bg-background/50 text-xs"
        >
          <SelectValue placeholder="Select a context..." />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {clusters.map((ctx) => (
              <SelectItem key={ctx.name} value={ctx.name}>
                {ctx.name}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>
  );
}

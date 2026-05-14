import { useState, useEffect, useCallback } from "react";
import { createTauriClient, listNamespaces } from "../lib/tauri";
import type { NamespaceSummary } from "../lib/types";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { TimestampText } from "@/components/TimestampText";

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
    return <div className="text-sm text-muted-foreground">Select a cluster context first</div>;
  }

  if (loading) {
    return <div className="flex flex-col text-sm text-muted-foreground">Loading namespaces...</div>;
  }

  if (error) {
    return (
      <div className="flex flex-col gap-2 text-sm text-destructive">
        Error: {error}
        <Button type="button" variant="outline" size="sm" onClick={loadNamespaces}>
          Retry
        </Button>
      </div>
    );
  }

  if (namespaces.length === 0) {
    return <div className="text-sm text-muted-foreground">No namespaces found</div>;
  }

  const allSelected = namespaces.length > 0 && selectedNamespaces.length === namespaces.length;

  return (
    <div className="flex flex-col">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
          Namespaces
        </h3>
        <Button
          onClick={handleToggleAll}
          type="button"
          variant="outline"
          size="sm"
          className="h-7 px-2 text-[0.625rem]"
        >
          {allSelected ? "Deselect All" : "Select All"}
        </Button>
      </div>
      <ul className="m-0 list-none p-0">
        {namespaces.map((ns) => (
          <li
            key={ns.name}
            className={cn(
              "cursor-pointer rounded-md p-2 text-sm transition-colors hover:bg-accent",
              selectedNamespaces.includes(ns.name) && "bg-accent",
            )}
          >
            <label className="flex cursor-pointer items-center gap-2">
              <input
                type="checkbox"
                checked={selectedNamespaces.includes(ns.name)}
                onChange={() => handleToggleOne(ns.name)}
                className="accent-primary"
              />
              <span className="flex-1">{ns.name}</span>
              <TimestampText
                relative={ns.age}
                exact={ns.createdAt}
                className="text-xs text-muted-foreground outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
              />
            </label>
          </li>
        ))}
      </ul>
    </div>
  );
}

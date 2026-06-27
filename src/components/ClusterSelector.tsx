import { useEffect, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { createTauriClient, listKubeContexts } from "../lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
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
  value?: string;
  onClusterChange: (cluster: string) => void;
}

export function ClusterSelector({ value, onClusterChange }: ClusterSelectorProps) {
  const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
  const {
    data: clusters = [],
    isPending: loading,
    isError,
    error,
    refetch,
  } = useQuery({
    queryKey: queryKeys.kubeContexts(kubeconfigEnvVar),
    queryFn: () => listKubeContexts(createTauriClient(), kubeconfigEnvVar),
  });

  const preferredContextName = useMemo(
    () => clusters.find((ctx) => ctx.isCurrent)?.name ?? clusters[0]?.name ?? "",
    [clusters],
  );
  const selectedValue = value ?? preferredContextName;

  useEffect(() => {
    if (!preferredContextName) return;
    const selectedStillExists = clusters.some((ctx) => ctx.name === value);
    if (!value || !selectedStillExists) {
      onClusterChange(preferredContextName);
    }
  }, [clusters, onClusterChange, preferredContextName, value]);

  const handleChange = (value: string) => {
    onClusterChange(value);
  };

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        Loading contexts…
      </div>
    );
  }

  if (isError) {
    const errorMessage =
      error instanceof Error ? error.message : "Failed to load contexts";
    return (
      <div className="flex items-center gap-2 text-xs text-destructive">
        Error: {errorMessage}
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void refetch()}
        >
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
        className="whitespace-nowrap text-xs font-bold uppercase tracking-wide text-muted-foreground"
      >
        Cluster Context:
      </span>
      <Select
        value={selectedValue}
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

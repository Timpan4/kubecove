import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { createTauriClient, detectArgoCD, listArgoApplications } from "../../lib/tauri";
import type { ArgoApplicationSummary } from "../../lib/types";

interface ArgoCDPanelProps {
  clusterContext: string;
  selectedArgoApp: ArgoApplicationSummary | null;
  onAppSelect: (app: ArgoApplicationSummary) => void;
  selectedArgoKind: string | null;
}

type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";

function StatusChip({ value, variant = "neutral" }: { value: string | null | undefined; variant?: ChipVariant }) {
  if (!value) return null;
  return (
    <span className={`chip chip-${variant}`}>
      {value}
    </span>
  );
}

function syncStatusVariant(status: string | null): ChipVariant {
  if (status === "Synced") return "success";
  if (status === "OutOfSync") return "warning";
  if (status === "Unknown") return "neutral";
  return "neutral";
}

function healthStatusVariant(status: string | null): ChipVariant {
  if (status === "Healthy") return "success";
  if (status === "Degraded" || status === "Missing") return "error";
  if (status === "Progressing" || status === "Unknown") return "warning";
  return "neutral";
}

export function ArgoCDPanel({ clusterContext, selectedArgoApp, onAppSelect, selectedArgoKind }: ArgoCDPanelProps) {
  const client = useMemo(() => createTauriClient(), []);
  const [search, setSearch] = useState("");

  const { data: argoDetected, isPending: detectPending } = useQuery({
    queryKey: ["argo-detect", clusterContext],
    queryFn: () => detectArgoCD(client, clusterContext),
    enabled: !!clusterContext,
    staleTime: 60_000,
  });

  // Applications is the only supported Argo kind for now
  const isApps = selectedArgoKind === "Applications";

  const { data: apps, isPending: appsPending, isError: appsError, error: appsErr } = useQuery({
    queryKey: ["argo-apps", clusterContext],
    queryFn: () => listArgoApplications(client, clusterContext),
    enabled: !!clusterContext && argoDetected === true && isApps,
    staleTime: 30_000,
  });

  if (detectPending) {
    return (
      <div className="resource-list-state">
        <span className="loading-indicator">Checking for Argo CD...</span>
      </div>
    );
  }

  if (argoDetected === false) {
    return (
      <div className="resource-list-state empty-state">
        <span>Argo CD not detected in this cluster</span>
      </div>
    );
  }

  // Show read-only placeholder for unsupported Argo kinds (ApplicationSets, AppProjects)
  if (selectedArgoKind && selectedArgoKind !== "Applications") {
    return (
      <div className="resource-list-state empty-state">
        <span>{selectedArgoKind} is not yet supported</span>
      </div>
    );
  }

  if (appsPending) {
    return (
      <div className="resource-list-state">
        <span className="loading-indicator">Loading Argo CD applications...</span>
      </div>
    );
  }

  if (appsError) {
    return (
      <div className="resource-list-state error-state">
        <span>Error: {appsErr instanceof Error ? appsErr.message : "Failed to load applications"}</span>
      </div>
    );
  }

  // apps is undefined when selectedArgoKind is null (Argo section header, query disabled)
  if (!apps) {
    return (
      <div className="resource-list-state empty-state">
        <span>Select an Argo CD resource type</span>
      </div>
    );
  }

  const filteredApps = search.trim()
    ? apps.filter((app) =>
        app.name.toLowerCase().includes(search.toLowerCase()) ||
        app.project?.toLowerCase().includes(search.toLowerCase()) ||
        app.sourceRepo?.toLowerCase().includes(search.toLowerCase())
      )
    : apps;

  return (
    <div className="argo-panel">
      <div className="resource-list-toolbar">
        <input
          className="resource-search-input"
          type="text"
          placeholder="Search by name, project, repo..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        {search && (
          <button className="clear-filter-btn" onClick={() => setSearch("")}>
            Clear
          </button>
        )}
      </div>

      <table className="resource-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Project</th>
            <th>Sync Status</th>
            <th>Health</th>
            <th>Destination</th>
            <th>Repo</th>
            <th>Revision</th>
            <th>Age</th>
          </tr>
        </thead>
        <tbody>
          {filteredApps.length === 0 ? (
            <tr>
              <td colSpan={8} className="empty-page-state">
                No applications found
              </td>
            </tr>
          ) : (
            filteredApps.map((app) => {
              const isSelected =
                selectedArgoApp !== null &&
                app.name === selectedArgoApp.name &&
                app.namespace === selectedArgoApp.namespace;
              return (
                <tr
                  key={app.name}
                  className={`resource-row${isSelected ? " selected" : ""}`}
                  onClick={() => onAppSelect(app)}
                >
                  <td>{app.name}</td>
                  <td>{app.project ?? "—"}</td>
                  <td>
                    <StatusChip value={app.syncStatus} variant={syncStatusVariant(app.syncStatus)} />
                  </td>
                  <td>
                    <StatusChip value={app.healthStatus} variant={healthStatusVariant(app.healthStatus)} />
                  </td>
                  <td>{app.destinationNamespace ?? "—"}</td>
                  <td title={app.sourceRepo ?? undefined}>{app.sourceRepo?.split("/").pop() ?? "—"}</td>
                  <td>{app.sourceRevision ?? "—"}</td>
                  <td>{app.age}</td>
                </tr>
              );
            })
          )}
        </tbody>
      </table>

      <div className="table-pagination">
        <span className="pagination-info">
          {filteredApps.length} {search ? "filtered" : "total"} applications
        </span>
      </div>
    </div>
  );
}
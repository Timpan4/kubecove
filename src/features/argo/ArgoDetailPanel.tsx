import { useQuery } from "@tanstack/react-query";
import { useState, useMemo } from "react";
import { getArgoApplicationDetails, createTauriClient } from "../../lib/tauri";
import type { ArgoApplicationSummary } from "../../lib/types";

interface ArgoDetailPanelProps {
  app: ArgoApplicationSummary;
  onClose: () => void;
}

type Tab = "details" | "yaml";

function DetailField({ label, value }: { label: string; value: string | undefined | null }) {
  if (!value) return null;
  return (
    <div className="detail-row">
      <span className="detail-key">{label}</span>
      <span className="detail-value">{value}</span>
    </div>
  );
}

function StatusChip({ value, label, variant }: { value: string | null | undefined; label: string; variant: string }) {
  if (!value) return null;
  return (
    <div className="detail-row">
      <span className="detail-key">{label}</span>
      <span className="detail-value">
        <span className={`chip chip-${variant}`}>{value}</span>
      </span>
    </div>
  );
}

export function ArgoDetailPanel({ app, onClose }: ArgoDetailPanelProps) {
  const [activeTab, setActiveTab] = useState<Tab>("details");
  const client = useMemo(() => createTauriClient(), []);

  const { data: details, isLoading: detailsLoading, isError: detailsError, error: detailsErr } = useQuery({
    queryKey: ["argo-app-details", app.cluster, app.name, app.namespace],
    queryFn: () =>
      getArgoApplicationDetails(client, app.cluster, app.name, app.namespace ?? undefined),
    enabled: !!app.cluster && !!app.name,
  });

  const getErrorMessage = (err: unknown): string => {
    if (err instanceof Error) return err.message;
    if (typeof err === "string") return err;
    return "Unknown error";
  };

  const formatMetadata = (metadata: Record<string, unknown>): Array<{ key: string; value: unknown }> => {
    const entries: Array<{ key: string; value: unknown }> = [];
    if (metadata.name) entries.push({ key: "Name", value: metadata.name });
    if (metadata.namespace) entries.push({ key: "Namespace", value: metadata.namespace });
    if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
    if (metadata.resourceVersion) entries.push({ key: "Resource Version", value: metadata.resourceVersion });
    if (metadata.creationTimestamp) entries.push({ key: "Created", value: metadata.creationTimestamp });
    if (metadata.labels) entries.push({ key: "Labels", value: JSON.stringify(metadata.labels, null, 2) });
    if (metadata.annotations) entries.push({ key: "Annotations", value: JSON.stringify(metadata.annotations, null, 2) });
    return entries;
  };

  return (
    <div className="right-panel">
      <div className="panel-header">
        <span className="panel-header-title">{app.name}</span>
        <button
          onClick={onClose}
          style={{
            background: "none",
            border: "none",
            color: "#888",
            cursor: "pointer",
            fontSize: "18px",
            padding: "0 4px",
          }}
          aria-label="Close panel"
        >
          ×
        </button>
      </div>
      <div className="panel-tabs">
        <button
          className={`panel-tab ${activeTab === "details" ? "active" : ""}`}
          onClick={() => setActiveTab("details")}
        >
          Details
        </button>
        <button
          className={`panel-tab ${activeTab === "yaml" ? "active" : ""}`}
          onClick={() => setActiveTab("yaml")}
        >
          YAML
        </button>
      </div>
      <div className="panel-body">
        {activeTab === "details" && (
          <>
            {detailsLoading && (
              <div className="loading-state">
                <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
                <span style={{ fontSize: "12px" }}>Loading...</span>
              </div>
            )}
            {detailsError && (
              <div className="error-state">
                <p>Error loading details: {getErrorMessage(detailsErr)}</p>
              </div>
            )}
            {!detailsLoading && !detailsError && details && (
              <>
                <div style={{ marginBottom: "16px", fontSize: "12px", color: "#888" }}>
                  Argo CD Application in {details.summary.namespace ?? "cluster-scoped"}
                </div>

                <div className="detail-section">
                  <div className="detail-section-title">Sync & Health</div>
                  <StatusChip
                    value={details.summary.syncStatus}
                    label="Sync Status"
                    variant={
                      details.summary.syncStatus === "Synced"
                        ? "success"
                        : details.summary.syncStatus === "OutOfSync"
                        ? "warning"
                        : "neutral"
                    }
                  />
                  <StatusChip
                    value={details.summary.healthStatus}
                    label="Health Status"
                    variant={
                      details.summary.healthStatus === "Healthy"
                        ? "success"
                        : details.summary.healthStatus === "Degraded" || details.summary.healthStatus === "Missing"
                        ? "error"
                        : "warning"
                    }
                  />
                </div>

                <div className="detail-section">
                  <div className="detail-section-title">Destination</div>
                  <DetailField label="Namespace" value={details.summary.destinationNamespace} />
                  <DetailField label="Server" value={details.summary.destinationServer} />
                </div>

                <div className="detail-section">
                  <div className="detail-section-title">Source</div>
                  <DetailField label="Repository" value={details.summary.sourceRepo} />
                  <DetailField label="Revision" value={details.summary.sourceRevision} />
                  <DetailField label="Project" value={details.summary.project} />
                </div>

                {details.status && Object.keys(details.status).length > 0 && (
                  <div className="detail-section">
                    <div className="detail-section-title">Status Details</div>
                    <pre style={{ fontSize: "12px", whiteSpace: "pre-wrap", color: "#e0e0e0" }}>
                      {JSON.stringify(details.status, null, 2)}
                    </pre>
                  </div>
                )}

                <div className="detail-section">
                  <div className="detail-section-title">Metadata</div>
                  {formatMetadata(details.metadata as Record<string, unknown>).map(({ key, value }) => (
                    <div key={key} className="detail-row">
                      <span className="detail-key">{key}</span>
                      <span className="detail-value">
                        {typeof value === "string" ? value : JSON.stringify(value)}
                      </span>
                    </div>
                  ))}
                </div>
              </>
            )}
          </>
        )}
        {activeTab === "yaml" && (
          <>
            {detailsLoading && (
              <div className="loading-state">
                <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
                <span style={{ fontSize: "12px" }}>Loading YAML...</span>
              </div>
            )}
            {detailsError && (
              <div className="error-state">
                <p>Error loading YAML: {getErrorMessage(detailsErr)}</p>
              </div>
            )}
            {!detailsLoading && !detailsError && details && (
              <pre className="yaml-block">{details.yaml}</pre>
            )}
          </>
        )}
      </div>
    </div>
  );
}
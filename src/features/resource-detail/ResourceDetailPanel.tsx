import { useQuery } from "@tanstack/react-query";
import { useState, useMemo } from "react";
import { getResourceDetails, getResourceYaml, createTauriClient } from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";

interface ResourceDetailPanelProps {
  resource: ResourceSummary;
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

function StatusChip({ value, label }: { value: string | undefined; label: string }) {
  if (!value) return null;
  const variant =
    value === "Running" || value === "Succeeded" || value === "Ready"
      ? "success"
      : value === "Pending" || value === "Terminating"
      ? "warning"
      : value === "Failed" || value === "Error"
      ? "error"
      : "neutral";
  return (
    <div className="detail-row">
      <span className="detail-key">{label}</span>
      <span className="detail-value">
        <span className={`chip chip-${variant}`}>{value}</span>
      </span>
    </div>
  );
}

function BadgeRow({ argoApp, helmRelease }: { argoApp?: string; helmRelease?: string }) {
  if (!argoApp && !helmRelease) return null;
  return (
    <div className="detail-row">
      <span className="detail-key">App</span>
      <span className="detail-value">
        <div className="detail-badges">
          {argoApp && <span className="badge badge-argo">Argo: {argoApp}</span>}
          {helmRelease && <span className="badge badge-helm">Helm: {helmRelease}</span>}
        </div>
      </span>
    </div>
  );
}

export function ResourceDetailPanel({ resource, onClose }: ResourceDetailPanelProps) {
  const [activeTab, setActiveTab] = useState<Tab>("details");
  const client = useMemo(() => createTauriClient(), []);

  const { data: details, isLoading: detailsLoading, isError: detailsError, error: detailsErr } = useQuery({
    queryKey: ["resource-details", resource.cluster, resource.kind, resource.name, resource.namespace],
    queryFn: () =>
      getResourceDetails(client, resource.cluster, resource.kind, resource.name, resource.namespace ?? undefined),
    enabled: !!resource.cluster && !!resource.kind && !!resource.name,
  });

  const { data: yaml, isLoading: yamlLoading, isError: yamlError, error: yamlErr } = useQuery({
    queryKey: ["resource-yaml", resource.cluster, resource.kind, resource.name, resource.namespace],
    queryFn: () =>
      getResourceYaml(client, resource.cluster, resource.kind, resource.name, resource.namespace ?? undefined),
    enabled: activeTab === "yaml" && !!resource.cluster && !!resource.kind && !!resource.name,
  });

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

  const getErrorMessage = (err: unknown): string => {
    if (err instanceof Error) return err.message;
    if (typeof err === "string") return err;
    return "Unknown error";
  };

  return (
    <div className="right-panel">
      <div className="panel-header">
        <span className="panel-header-title">
          {resource.name}
        </span>
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
                  {details.summary.kind} in {details.summary.namespace ?? "cluster-scoped"}
                </div>

                {/* Summary fields: status chips, owner, argo/helm */}
                <div className="detail-section">
                  <div className="detail-section-title">Status</div>
                  <StatusChip value={resource.status} label="Phase" />
                  <StatusChip value={resource.ready} label="Ready" />
                  {resource.restarts !== undefined && resource.restarts > 0 && (
                    <div className="detail-row">
                      <span className="detail-key">Restarts</span>
                      <span className="detail-value">
                        <span className={`chip chip-${resource.restarts > 5 ? "error" : "warning"}`}>
                          {resource.restarts}
                        </span>
                      </span>
                    </div>
                  )}
                </div>

                {resource.ownerRef && (
                  <DetailField label="Owner" value={resource.ownerRef} />
                )}

                <BadgeRow argoApp={resource.argoApp} helmRelease={resource.helmRelease} />

                {details.status && (
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
            {yamlLoading && (
              <div className="loading-state">
                <div className="loading-spinner" style={{ width: "16px", height: "16px", marginBottom: "8px" }}></div>
                <span style={{ fontSize: "12px" }}>Loading YAML...</span>
              </div>
            )}
            {yamlError && (
              <div className="error-state">
                <p>Error loading YAML: {getErrorMessage(yamlErr)}</p>
              </div>
            )}
            {!yamlLoading && !yamlError && yaml && (
              <pre className="yaml-block">{yaml}</pre>
            )}
          </>
        )}
      </div>
    </div>
  );
}
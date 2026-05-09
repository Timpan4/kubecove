interface ResourceDetailProps {
  resource: { kind: string; name: string; namespace: string | null } | null;
}

export function ResourceDetail({ resource }: ResourceDetailProps) {
  if (!resource) {
    return (
      <div className="resource-detail empty">
        <div className="empty-message">Select a resource to view details</div>
      </div>
    );
  }

  return (
    <div className="resource-detail">
      <h3 className="resource-detail-title">{resource.kind}: {resource.name}</h3>
      <div className="resource-detail-content">
        <div className="detail-field">
          <span className="detail-label">Kind:</span>
          <span className="detail-value">{resource.kind}</span>
        </div>
        <div className="detail-field">
          <span className="detail-label">Name:</span>
          <span className="detail-value">{resource.name}</span>
        </div>
        <div className="detail-field">
          <span className="detail-label">Namespace:</span>
          <span className="detail-value">{resource.namespace ?? "-"}</span>
        </div>
      </div>
    </div>
  );
}

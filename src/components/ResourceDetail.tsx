interface ResourceDetailProps {
  resource: { kind: string; name: string; namespace: string | null } | null;
}

export function ResourceDetail({ resource }: ResourceDetailProps) {
  if (!resource) {
    return (
      <div className="text-sm text-muted-foreground">
        <div>Select a resource to view details</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-base font-semibold">{resource.kind}: {resource.name}</h3>
      <div className="flex flex-col gap-2">
        <div className="flex gap-3 border-b py-1.5">
          <span className="min-w-[120px] text-xs font-medium text-muted-foreground">Kind:</span>
          <span className="text-xs text-foreground">{resource.kind}</span>
        </div>
        <div className="flex gap-3 border-b py-1.5">
          <span className="min-w-[120px] text-xs font-medium text-muted-foreground">Name:</span>
          <span className="text-xs text-foreground">{resource.name}</span>
        </div>
        <div className="flex gap-3 border-b py-1.5">
          <span className="min-w-[120px] text-xs font-medium text-muted-foreground">Namespace:</span>
          <span className="text-xs text-foreground">{resource.namespace ?? "-"}</span>
        </div>
      </div>
    </div>
  );
}

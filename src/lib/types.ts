export interface ClusterContext {
  name: string;
}

export interface NamespaceSummary {
  name: string;
  age: string;
}

export interface ResourceSummary {
  kind: string;
  cluster: string;
  name: string;
  namespace: string | null;
  age: string;
}

export interface ResourceDetails {
  kind: string;
  cluster: string;
  name: string;
  namespace: string | null;
  yaml: string;
}

export interface ResourceDetailsFull {
  summary: ResourceSummary;
  yaml: string;
  metadata: Record<string, unknown>;
  status?: Record<string, unknown>;
}

export interface AppError {
  message: string;
  kind: string;
}
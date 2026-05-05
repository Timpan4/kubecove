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

export interface AppError {
  message: string;
  kind: string;
}
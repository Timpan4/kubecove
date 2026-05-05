export type ClusterContext = {
  name: string;
  cluster?: string;
  user?: string;
  namespace?: string;
};

export type NamespaceSummary = {
  name: string;
  status?: string;
  age?: string;
};

export type ResourceSummary = {
  clusterContext: string;
  apiVersion: string;
  kind: string;
  namespace?: string;
  name: string;
  uid?: string;
  status?: string;
  age?: string;
  ready?: string;
  restarts?: number;
  owner?: string;
  argoApp?: string;
  helmRelease?: string;
  labels?: Record<string, string>;
};

export type ResourceDetails = {
  summary: ResourceSummary;
  yaml: string;
  metadata: unknown;
  status?: unknown;
};

export type AppErrorKind =
  | "kubeConfig"
  | "cluster"
  | "resource"
  | "serialization";

export type AppError = {
  kind: AppErrorKind;
  message: string;
};

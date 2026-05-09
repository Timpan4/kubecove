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
  status?: string;
  ready?: string;
  restarts?: number;
  ownerRef?: string;
  argoApp?: string;
  helmRelease?: string;
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

export const SUPPORTED_KINDS = [
  "Pod",
  "Deployment",
  "StatefulSet",
  "DaemonSet",
  "Service",
  "Ingress",
  "ConfigMap",
  "Secret",
  "PersistentVolumeClaim",
  "Job",
  "CronJob",
] as const;

export type SupportedKind = (typeof SUPPORTED_KINDS)[number];
import { invoke, InvokeOptions } from "@tauri-apps/api/core";
import type { ClusterContext, NamespaceSummary, ResourceSummary, ResourceDetails, AppError } from "./types";

export interface TauriClient {
  invoke<T>(cmd: string, args?: Record<string, unknown>, options?: InvokeOptions): Promise<T>;
}

export function createTauriClient(): TauriClient {
  return {
    invoke,
  };
}

export function createMockTauriClient(mockResponses: Record<string, unknown>): TauriClient {
  return {
    invoke: async <T>(cmd: string): Promise<T> => {
      if (cmd in mockResponses) {
        return mockResponses[cmd] as T;
      }
      throw new Error(`No mock response for command: ${cmd}`);
    },
  };
}

export async function listKubeContexts(client: TauriClient): Promise<ClusterContext[]> {
  return client.invoke<ClusterContext[]>("list_kube_contexts");
}

export async function listNamespaces(client: TauriClient, clusterContext: string): Promise<NamespaceSummary[]> {
  return client.invoke<NamespaceSummary[]>("list_namespaces", { clusterContext });
}

export async function listResources(
  client: TauriClient,
  clusterContext: string,
  kind: string,
  namespace?: string
): Promise<ResourceSummary[]> {
  return client.invoke<ResourceSummary[]>("list_resources", { clusterContext, kind, namespace });
}

export async function getResourceYaml(
  client: TauriClient,
  clusterContext: string,
  kind: string,
  name: string,
  namespace?: string
): Promise<ResourceDetails> {
  return client.invoke<ResourceDetails>("get_resource_yaml", { clusterContext, kind, name, namespace });
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "message" in value &&
    "kind" in value
  );
}
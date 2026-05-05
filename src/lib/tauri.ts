import { invoke } from "@tauri-apps/api/core";

import type { ClusterContext } from "./types";

const LIST_KUBE_CONTEXTS_COMMAND = "list_kube_contexts";

type ListKubeContextsInvoke = (
  command: typeof LIST_KUBE_CONTEXTS_COMMAND,
) => Promise<ClusterContext[]>;

export type TauriClient = {
  listKubeContexts: () => Promise<ClusterContext[]>;
};

export function createTauriClient(
  invokeCommand: ListKubeContextsInvoke = (command) =>
    invoke<ClusterContext[]>(command),
): TauriClient {
  return {
    listKubeContexts: () => invokeCommand(LIST_KUBE_CONTEXTS_COMMAND),
  };
}

const tauriClient = createTauriClient();

export function listKubeContexts(): Promise<ClusterContext[]> {
  return tauriClient.listKubeContexts();
}

import { expect, test } from "bun:test";

import { createTauriClient } from "../src/lib/tauri";

test("listKubeContexts invokes the typed Tauri command", async () => {
  const contexts = [
    {
      name: "kind-dev",
      cluster: "kind-dev",
      user: "kind-dev",
      namespace: "default",
    },
  ];
  const calls: string[] = [];
  const client = createTauriClient(async (command) => {
    calls.push(command);
    return contexts;
  });

  await expect(client.listKubeContexts()).resolves.toEqual(contexts);
  expect(calls).toEqual(["list_kube_contexts"]);
});

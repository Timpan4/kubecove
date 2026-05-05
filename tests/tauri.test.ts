import { describe, test, expect } from "bun:test";
import { createMockTauriClient, listKubeContexts, listNamespaces, isAppError } from "../src/lib/tauri";
import type { ClusterContext, NamespaceSummary } from "../src/lib/types";

describe("createMockTauriClient", () => {
  test("returns mock response for known command", async () => {
    const mockContexts: ClusterContext[] = [{ name: "minikube" }, { name: "docker-desktop" }];
    const client = createMockTauriClient({
      list_kube_contexts: mockContexts,
    });

    const result = await listKubeContexts(client);
    expect(result).toEqual(mockContexts);
  });

  test("throws for unknown command", async () => {
    const client = createMockTauriClient({});

    let threw = false;
    try {
      await listKubeContexts(client);
    } catch {
      threw = true;
    }
    expect(threw).toBe(true);
  });
});

describe("listKubeContexts", () => {
  test("returns cluster contexts from client", async () => {
    const mockContexts: ClusterContext[] = [{ name: "minikube" }];
    const mockClient = createMockTauriClient({ list_kube_contexts: mockContexts });

    const result = await listKubeContexts(mockClient);
    expect(result).toEqual(mockContexts);
    expect(result[0].name).toBe("minikube");
  });
});

describe("listNamespaces", () => {
  test("passes cluster_context to client", async () => {
    const mockNamespaces: NamespaceSummary[] = [
      { name: "default", age: "2024-01-01T00:00:00Z" },
    ];
    const client = createMockTauriClient({ list_namespaces: mockNamespaces });

    const result = await listNamespaces(client, "minikube");
    expect(result).toEqual(mockNamespaces);
  });
});

describe("isAppError", () => {
  test("returns true for valid AppError", () => {
    const err = { message: "test", kind: "cluster" };
    expect(isAppError(err)).toBe(true);
  });

  test("returns false for non-AppError object", () => {
    expect(isAppError({ message: "test" })).toBe(false);
    expect(isAppError({ kind: "cluster" })).toBe(false);
    expect(isAppError(null)).toBe(false);
  });
});
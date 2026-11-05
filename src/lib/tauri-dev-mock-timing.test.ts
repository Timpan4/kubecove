import { expect, test } from "bun:test";
import { MockTiming } from "./tauri-dev-mock-timing";

// Failure modes are recorded in e2e/parity/mock-timing.md before this implementation.
test("a dirty in-flight load cancels earlier waiters and clean callers join", async () => {
    const timing = new MockTiming();
    timing.control({ operation: "hold", command: "list_resource_topology", context: "mock-dev" });
    const args = { clusterContext: "mock-dev" };
    const first = timing.read("list_resource_topology", args, () => "old");
    const cancelled = first.catch((error: Error) => error);
    timing.control({ operation: "dirty", command: "list_resource_topology", context: "mock-dev" });
    const second = timing.read("list_resource_topology", args, () => "new");
    const joined = timing.read("list_resource_topology", args, () => "wrong");
    timing.control({ operation: "release", command: "list_resource_topology", context: "mock-dev" });
    expect(await cancelled).toMatchObject({ kind: "cancelled" });
    expect(await second).toBe("new");
    expect(await joined).toBe("new");
});

test("independent reads release out of order and a failed read can retry", async () => {
    const timing = new MockTiming();
    const a = { command: "list_resource_topology", context: "a" };
    const b = { ...a, context: "b" };
    timing.control({ ...a, operation: "hold" });
    timing.control({ ...b, operation: "hold" });
    const first = timing.read(a.command, { clusterContext: "a" }, () => "a");
    const failed = first.catch((error: Error) => error);
    const second = timing.read(b.command, { clusterContext: "b" }, () => "b");
    timing.control({ ...b, operation: "release" });
    expect(await second).toBe("b");
    timing.control({ ...a, operation: "failOnce", kind: "forbidden" });
    expect(await failed).toMatchObject({ kind: "forbidden" });
    expect(await timing.read(a.command, { clusterContext: "a" }, () => "retry")).toBe("retry");
});

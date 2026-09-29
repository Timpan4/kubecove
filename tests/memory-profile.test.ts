import { describe, expect, it } from "bun:test";
import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import { collectQueryMemory } from "../src/lib/memory-profile";

describe("redacted query memory observations", () => {
	it("reports empty topology and metric collections as zero items", () => {
		const client = new QueryClient();
		client.setQueryData(["resource-topology"], { nodes: [], edges: [], warnings: [] });
		client.setQueryData(["resource-metrics"], { nodes: [], pods: [], workloads: [], warnings: [] });
		expect(collectQueryMemory(client).roots.every((root) => root.items === 0)).toBe(true);
		client.clear();
	});
	it("counts queries during collection waits without serializing their payload", () => {
		const client = new QueryClient();
		let serialized = false;
		client.setQueryData(["resources"], { toJSON() { serialized = true; return []; } });
		const result = collectQueryMemory(client, false);
		expect(serialized).toBe(false);
		expect(result.roots[0].jsonBytes).toBeNull();
		client.clear();
	});
	it("counts inactive payloads in UTF-8 bytes without returning identifiers", () => {
		const client = new QueryClient();
		client.setQueryData(["resources", "private-context", "private-namespace"], [{ name: "private-resource-é" }]);
		client.setQueryData(["private-context"], ["private-payload"]);
		const result = collectQueryMemory(client);
		const resources = result.roots.find((root) => root.root === "resources");
		expect(resources?.queries).toBe(1);
		expect(resources?.inactive).toBe(1);
		expect(resources?.items).toBe(1);
		expect(resources?.jsonBytes).toBe(new TextEncoder().encode(JSON.stringify([{ name: "private-resource-é" }])).byteLength);
		expect(result.roots.find((root) => root.root === "other")?.queries).toBe(1);
		expect(JSON.stringify(result)).not.toContain("private");
		client.clear();
	});

	it("counts observed empty queries and marks unserializable payload weight unavailable", () => {
		const client = new QueryClient();
		const observer = new QueryObserver(client, { queryKey: ["resource-metrics"], queryFn: async () => null, enabled: false });
		const unsubscribe = observer.subscribe(() => {});
		client.setQueryData(["resources"], { value: BigInt(1) });
		const result = collectQueryMemory(client);
		const metrics = result.roots.find((root) => root.root === "resource-metrics");
		expect(metrics?.queries).toBe(1);
		expect(metrics?.observed).toBe(1);
		expect(metrics?.items).toBe(0);
		expect(result.roots.find((root) => root.root === "resources")?.jsonBytes).toBeNull();
		expect(result.roots.find((root) => root.root === "resources")?.unavailablePayloads).toBe(1);
		unsubscribe();
		client.clear();
	});
});

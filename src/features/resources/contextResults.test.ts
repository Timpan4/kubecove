import { expect, test } from "bun:test";
import type { ResourceScopeResult } from "@/lib/types";
import { type ContextScopeRead, mergeContextResults } from "./contextResults";

// Rank order, partial failure and watch isolation are covered by the
// resources.multi-context-* parity scenarios. These cover what the mocks cannot
// reach: a failed refresh, every context failing, and placeholder versions.
const request = { kind: "Deployment", namespace: "payments" };
function result(cluster: string, version: string): ResourceScopeResult {
	return {
		resources: [{ cluster, kind: "Deployment", name: "payments-api", namespace: "payments", age: "1d", health: "healthy" }],
		completedRequests: [request],
		resourceVersions: [{ request, resourceVersion: version }],
		failures: [],
	};
}
const settled = (clusterContext: string, version: string): ContextScopeRead => ({
	clusterContext, data: result(clusterContext, version), error: null, isError: false, isPending: false, isPlaceholderData: false,
});
const failed = (clusterContext: string, data?: ResourceScopeResult): ContextScopeRead => ({
	clusterContext, data, error: new Error("Context unavailable"), isError: true, isPending: false, isPlaceholderData: false,
});

test("a failed refresh keeps the context's earlier rows and marks its warning stale", () => {
	const merged = mergeContextResults([settled("a", "1"), failed("b", result("b", "7"))]);
	expect(merged.data?.resources.map((row) => row.cluster)).toEqual(["a", "b"]);
	expect(merged.contextFailures).toEqual([{ clusterContext: "b", message: "Context unavailable", stale: true }]);
	expect(merged.isSuccess).toBe(true);
	expect(merged.error).toBeNull();
});

test("every context failing is one error that names each context in rank order", () => {
	const merged = mergeContextResults([failed("a"), failed("b")]);
	expect(merged.isError).toBe(true);
	expect(merged.data).toBeUndefined();
	expect(String(merged.error)).toContain('Context "a", operation "resource discovery": Context unavailable\nContext "b"');
});

test("one context keeps its own error and result identity", () => {
	const only = settled("a", "1");
	expect(mergeContextResults([only]).data).toBe(only.data);
	const error = failed("a");
	expect(mergeContextResults([error]).error).toBe(error.error);
	expect(mergeContextResults([error]).contextFailures).toEqual([]);
});

test("placeholder or loading contexts hold their watches while settled contexts start at their own version", () => {
	const placeholder = { ...settled("b", "old"), isPlaceholderData: true };
	const loading: ContextScopeRead = { clusterContext: "c", data: undefined, error: null, isError: false, isPending: true, isPlaceholderData: false };
	const merged = mergeContextResults([settled("a", "1"), placeholder, loading]);
	expect(merged.versions.get("a")?.map((version) => version.resourceVersion)).toEqual(["1"]);
	expect(merged.versions.get("b")).toBeNull();
	expect(merged.versions.get("c")).toBeNull();
	expect(merged.isPending).toBe(true);
	expect(merged.isSuccess).toBe(false);
});

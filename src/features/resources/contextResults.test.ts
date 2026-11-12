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

const loading = (clusterContext: string): ContextScopeRead => ({
	clusterContext, data: undefined, error: null, isError: false, isPending: true, isPlaceholderData: false,
});

test("placeholder or loading contexts hold their watches while settled contexts start at their own version", () => {
	const placeholder = { ...settled("b", "old"), isPlaceholderData: true };
	const merged = mergeContextResults([settled("a", "1"), placeholder, loading("c")]);
	expect(merged.versions.get("a")?.map((version) => version.resourceVersion)).toEqual(["1"]);
	expect(merged.versions.get("b")).toBeNull();
	expect(merged.versions.get("c")).toBeNull();
});

test("a context still loading leaves the rows of contexts that answered and is named until it answers", () => {
	const merged = mergeContextResults([loading("a"), settled("b", "1"), failed("c")]);
	expect(merged.isPending).toBe(false);
	expect(merged.isSuccess).toBe(true);
	expect(merged.data?.resources.map((row) => row.cluster)).toEqual(["b"]);
	expect(merged.loadingContexts).toEqual(["a"]);
	expect(merged.shownContexts).toEqual(["b"]);
	expect(mergeContextResults([settled("a", "1"), settled("b", "2")]).loadingContexts).toEqual([]);
});

test("the table waits while a context loads and none has rows, and every failure but a loading context is no page error", () => {
	expect(mergeContextResults([loading("a"), loading("b")]).isPending).toBe(true);
	const waiting = mergeContextResults([loading("a"), failed("b")]);
	expect(waiting.isPending).toBe(true);
	expect(waiting.isError).toBe(false);
	expect(waiting.data).toBeUndefined();
	expect(waiting.loadingContexts).toEqual([]);
	expect(waiting.contextFailures.map((failure) => failure.clusterContext)).toEqual(["b"]);
});

test("a context whose stale rows stay after a failed refresh counts as shown", () => {
	expect(mergeContextResults([settled("a", "1"), failed("b", result("b", "7")), failed("c")]).shownContexts).toEqual(["a", "b"]);
});

import { describe, expect, test } from "bun:test";
import { isTransientQueryError, queryRetry } from "../src/lib/query-retry";

describe("query retry policy", () => {
	test("does not retry deterministic backend error kinds", () => {
		for (const kind of ["validation", "serialization", "fieldManagerConflict"]) {
			expect(isTransientQueryError({ message: "request failed", kind })).toBe(false);
			expect(queryRetry(0, { message: "request failed", kind })).toBe(false);
		}
	});

	test("does not retry deterministic cluster messages", () => {
		const errors = [
			{ message: "deployments.apps is forbidden: User cannot list", kind: "cluster" },
			{ message: "pods api returned 403 Forbidden", kind: "cluster" },
			{ message: "services \"api\" not found", kind: "cluster" },
			{ message: "resource request failed with 404", kind: "cluster" },
		];

		for (const error of errors) {
			expect(isTransientQueryError(error)).toBe(false);
			expect(queryRetry(0, error)).toBe(false);
		}
	});

	test("retries transient errors only for the first two failures", () => {
		const error = { message: "connection reset by peer", kind: "cluster" };

		expect(queryRetry(0, error)).toBe(true);
		expect(queryRetry(1, error)).toBe(true);
		expect(queryRetry(2, error)).toBe(false);
	});

	test("handles Error and string inputs safely", () => {
		expect(isTransientQueryError(new Error("network timeout"))).toBe(true);
		expect(isTransientQueryError(new Error("404 Not Found"))).toBe(false);
		expect(isTransientQueryError("temporary dns failure")).toBe(true);
		expect(isTransientQueryError("forbidden")).toBe(false);
	});
});

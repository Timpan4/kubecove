import type { ResourceSummary } from "@/lib/types";
import { guardedOperationBlocker, guardedOperations } from "./operations-model";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toContain(expected: unknown): void;
};

function resource(kind: string): ResourceSummary {
	return {
		cluster: "kind-dev",
		kind,
		name: "api",
		namespace: "payments",
		age: "1h",
		health: "healthy",
	};
}

describe("guarded operations", () => {
	test("lists exact supported operations for a Deployment", () => {
		const operations = guardedOperations(resource("Deployment"));

		expect(operations.available.map((operation) => operation.id)).toEqual(["scale", "restart"]);
		expect(operations.available[0]?.scope).toContain("this exact selected Deployment resource only");
	});

	test("identifies kind as blocker when no guarded operation exists", () => {
		const operations = guardedOperations(resource("Service"));

		expect(operations.available).toEqual([]);
		expect(operations.blocker).toContain("Service has no supported guarded operation");
	});

	test("classifies permission failures returned by operation preview", () => {
		expect(guardedOperationBlocker("deployments.apps is forbidden")).toBe("permission");
	});
});

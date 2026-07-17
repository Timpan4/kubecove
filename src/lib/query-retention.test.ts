import { QueryClient, QueryObserver } from "@tanstack/svelte-query";
import {
	configureLargeQueryRetention,
	LARGE_QUERY_GC_TIME_MS,
	LARGE_QUERY_ROOTS,
} from "./query-retention";

declare function afterEach(fn: () => void): void;
declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
};
declare const jest: {
	useFakeTimers(): void;
	useRealTimers(): void;
	advanceTimersByTime(milliseconds: number): void;
};

afterEach(() => {
	jest.useRealTimers();
});

describe("large query retention", () => {
	test("expires inactive large queries at 90 seconds without changing stale or default retention", () => {
		jest.useFakeTimers();
		const queryClient = new QueryClient({
			defaultOptions: { queries: { gcTime: 300_000, staleTime: 30_000 } },
		});
		configureLargeQueryRetention(queryClient);

		const inactiveKeys = LARGE_QUERY_ROOTS.map((root) => [root, "inactive"] as const);
		const unrelatedKey = ["resource-details", "default"] as const;
		for (const key of inactiveKeys) queryClient.setQueryData(key, "inactive");
		queryClient.setQueryData(unrelatedKey, "default");

		for (const key of inactiveKeys) {
			expect(queryClient.getQueryCache().find({ queryKey: key })?.options.gcTime).toBe(
				LARGE_QUERY_GC_TIME_MS,
			);
		}
		expect(queryClient.defaultQueryOptions({ queryKey: inactiveKeys[0] }).staleTime).toBe(
			30_000,
		);
		expect(queryClient.defaultQueryOptions({ queryKey: unrelatedKey }).gcTime).toBe(300_000);
		jest.advanceTimersByTime(LARGE_QUERY_GC_TIME_MS - 1);
		expect(inactiveKeys.every((key) => queryClient.getQueryData(key) === "inactive")).toBe(
			true,
		);
		expect(queryClient.getQueryData(unrelatedKey)).toBe("default");

		jest.advanceTimersByTime(1);
		expect(inactiveKeys.every((key) => queryClient.getQueryData(key) === undefined)).toBe(
			true,
		);
		expect(queryClient.getQueryData(unrelatedKey)).toBe("default");

		queryClient.clear();
	});

	test("reopening before expiry keeps cached resource data for a new inactive window", () => {
		jest.useFakeTimers();
		const queryClient = new QueryClient();
		configureLargeQueryRetention(queryClient);
		const key = ["resources", "reopened"] as const;
		queryClient.setQueryData(key, "cached");

		jest.advanceTimersByTime(LARGE_QUERY_GC_TIME_MS - 1);
		const observer = new QueryObserver(queryClient, { queryKey: key, enabled: false });
		const unsubscribe = observer.subscribe(() => {});
		expect(observer.getCurrentResult().data).toBe("cached");

		jest.advanceTimersByTime(1);
		expect(queryClient.getQueryData(key)).toBe("cached");
		unsubscribe();

		jest.advanceTimersByTime(LARGE_QUERY_GC_TIME_MS - 1);
		expect(queryClient.getQueryData(key)).toBe("cached");
		jest.advanceTimersByTime(1);
		expect(queryClient.getQueryData(key)).toBe(undefined);

		queryClient.clear();
	});
});

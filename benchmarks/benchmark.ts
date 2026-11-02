import { test } from "vitest";

/**
 * Registers a test that runs one named benchmark. Vitest 5 runs benchmarks
 * through the `bench` test fixture, and one benchmark per test keeps each
 * result named after its benchmark.
 *
 * `timeout: 0` turns off the test timeout, as Vitest 4 `bench()` had none.
 * Vitest 5 raises a benchmark project's `testTimeout` to at least 60 s, and
 * the largest suites take minutes under CodSpeed instrumentation.
 */
export function benchmark(name: string, fn: () => void): void {
	test(name, { timeout: 0 }, async ({ bench }) => {
		await bench(name, fn).run();
	});
}

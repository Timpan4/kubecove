import { test } from "vitest";

/**
 * Registers a test that runs one named benchmark. Vitest 5 runs benchmarks
 * through the `bench` test fixture, and one benchmark per test keeps each
 * result named after its benchmark.
 */
export function benchmark(name: string, fn: () => void): void {
	test(name, async ({ bench }) => {
		await bench(name, fn).run();
	});
}

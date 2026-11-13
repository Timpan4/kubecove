import { fileURLToPath } from "node:url";
import codspeedPlugin from "@codspeed/vitest-plugin";
import { defineConfig } from "vitest/config";
import { benchmarkSelection } from "./scripts/codspeed-benchmark-suites.mjs";

// Dedicated Vitest configuration for CodSpeed benchmarks.
// Unit tests continue to run through `bun test`; this config only powers the
// `*.bench.ts` files under `benchmarks/`. CI selects a complete, disjoint
// duration-balanced group through `benchmarkSelection()`, including name filters.
const { testNamePattern, ...benchmark } = benchmarkSelection();
export default defineConfig({
	plugins: [codspeedPlugin()],
	resolve: {
		alias: {
			"@": fileURLToPath(new URL("./src", import.meta.url)),
		},
	},
	test: {
		testNamePattern,
		benchmark,
	},
});

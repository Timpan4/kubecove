import { fileURLToPath } from "node:url";
import codspeedPlugin from "@codspeed/vitest-plugin";
import { defineConfig } from "vitest/config";

// Dedicated Vitest configuration for CodSpeed benchmarks.
// Unit tests continue to run through `bun test`; this config only powers the
// `*.bench.ts` files under `benchmarks/` so the CodSpeed instrumentation can
// measure the performance-critical pure helpers.
export default defineConfig({
	plugins: [codspeedPlugin()],
	resolve: {
		alias: {
			"@": fileURLToPath(new URL("./src", import.meta.url)),
		},
	},
	test: {
		benchmark: {
			include: ["benchmarks/**/*.bench.ts"],
		},
	},
});

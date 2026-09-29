import { join } from "node:path";
import { browser } from "@wdio/globals";
import type { Capabilities, Options } from "@wdio/types";
import { LARGE_QUERY_GC_TIME_MS } from "../src/lib/query-retention";

const profiling = process.env.KUBECOVE_PROFILE === "1";
const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
type E2eConfig = Options.Testrunner & {
	capabilities: Capabilities.TestrunnerCapabilities;
	autoCompileOpts: {
		autoCompile: boolean;
		tsNodeOpts: { project: string; transpileOnly: boolean };
	};
};

export const config: E2eConfig = {
	runner: "local",
	logLevel: profiling ? "silent" : "warn",
	specs: profiling ? (process.env.KUBECOVE_MEMORY_FIXTURE ? ["./specs/profile/memory.e2e.ts"] : ["./specs/profile/**/*.e2e.ts"]) : process.env.KUBECOVE_E2E_SMOKE
		? ["./specs/desktop/**/*.e2e.ts"]
		: ["./specs/real/**/*.e2e.ts"],
	maxInstances: 1,
	framework: "mocha",
	reporters: profiling ? [] : [["spec", artifacts ? { outputDir: artifacts } : {}]],
	// The memory scenario adds the real collection interval to the existing scenario budget.
	mochaOpts: { ui: "bdd", timeout: 90_000 + (process.env.KUBECOVE_MEMORY_FIXTURE ? LARGE_QUERY_GC_TIME_MS : 0) },
	services: [["@wdio/tauri-service", { appBinaryPath: process.env.KUBECOVE_E2E_BINARY ?? join("src-tauri", "target", "debug", process.platform === "win32" ? "kubecove.exe" : "kubecove"), driverProvider: "embedded", embeddedPort: 4445, captureBackendLogs: !profiling, captureFrontendLogs: !profiling }]],
	capabilities: [{ browserName: "tauri" }],
	autoCompileOpts: { autoCompile: true, tsNodeOpts: { project: "./tsconfig.json", transpileOnly: true } },
	afterTest: async (_test, _context, result: { passed: boolean }) => {
		if (!result.passed && artifacts) await browser.saveScreenshot(join(artifacts, `failure-${Date.now()}.png`));
	},
};

if (artifacts && !profiling) config.outputDir = artifacts;

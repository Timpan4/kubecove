import { join } from "node:path";
import { browser } from "@wdio/globals";
import type { Capabilities, Options } from "@wdio/types";

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
	logLevel: "warn",
	specs: process.env.KUBECOVE_E2E_SMOKE
		? ["./specs/desktop/**/*.e2e.ts"]
		: ["./specs/real/**/*.e2e.ts"],
	maxInstances: 1,
	framework: "mocha",
	reporters: [["spec", artifacts ? { outputDir: artifacts } : {}]],
	mochaOpts: { ui: "bdd", timeout: 90_000 },
	services: [["@wdio/tauri-service", { appBinaryPath: process.env.KUBECOVE_E2E_BINARY ?? join("src-tauri", "target", "debug", process.platform === "win32" ? "kubecove.exe" : "kubecove"), driverProvider: "embedded", captureBackendLogs: true, captureFrontendLogs: true }]],
	capabilities: [{ browserName: "tauri" }],
	autoCompileOpts: { autoCompile: true, tsNodeOpts: { project: "./tsconfig.json", transpileOnly: true } },
	afterTest: async (_test, _context, result: { passed: boolean }) => {
		if (!result.passed && artifacts) await browser.saveScreenshot(join(artifacts, `failure-${Date.now()}.png`));
	},
};

if (artifacts) config.outputDir = artifacts;

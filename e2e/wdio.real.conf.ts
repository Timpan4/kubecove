import { join } from "node:path";
import { browser } from "@wdio/globals";

const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;

export const config = {
	runner: "local",
	logLevel: "warn",
	...(artifacts ? { outputDir: artifacts } : {}),
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
	afterTest: async (_test: unknown, _context: unknown, result: { passed: boolean }) => {
		if (!result.passed && artifacts) await browser.saveScreenshot(join(artifacts, `failure-${Date.now()}.png`));
	},
};

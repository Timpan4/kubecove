import { join } from "node:path";
import { browser } from "@wdio/globals";

const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;

export const config = {
	runner: "local",
	logLevel: "warn",
	...(artifacts ? { outputDir: artifacts } : {}),
	specs: ["./specs/fast/**/*.e2e.ts"],
	maxInstances: 1,
	framework: "mocha",
	reporters: [["spec", artifacts ? { outputDir: artifacts } : {}]],
	mochaOpts: { ui: "bdd", timeout: 30_000 },
	capabilities: [{ browserName: "chrome", "goog:chromeOptions": { args: ["--headless=new", "--no-sandbox"] } }],
	baseUrl: process.env.E2E_FAST_URL ?? "http://127.0.0.1:1420",
	autoCompileOpts: { autoCompile: true, tsNodeOpts: { project: "./tsconfig.json", transpileOnly: true } },
	afterTest: async (_test: unknown, _context: unknown, result: { passed: boolean }) => {
		if (!result.passed && artifacts) await browser.saveScreenshot(join(artifacts, `failure-${Date.now()}.png`));
	},
};

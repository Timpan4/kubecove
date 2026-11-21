import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { browser } from "@wdio/globals";
import type { Capabilities, Options } from "@wdio/types";

const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
const profiles = process.env.E2E_FAST_CHROME_PROFILES;
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
	specs: ["./specs/fast/**/*.e2e.ts"],
	maxInstances: 1,
	framework: "mocha",
	reporters: [["spec", artifacts ? { outputDir: artifacts } : {}]],
	mochaOpts: { ui: "bdd", timeout: 30_000 },
	capabilities: [{ browserName: "chrome", "goog:chromeOptions": { args: ["--headless=new", "--no-sandbox"] } }],
	beforeSession: (_config, capabilities) => {
		if (!profiles || !("goog:chromeOptions" in capabilities)) return;
		const options = capabilities["goog:chromeOptions"];
		if (options && "args" in options) options.args?.push(`--user-data-dir=${join(profiles, randomUUID())}`);
	},
	baseUrl: process.env.E2E_FAST_URL ?? "http://127.0.0.1:1420",
	autoCompileOpts: { autoCompile: true, tsNodeOpts: { project: "./tsconfig.json", transpileOnly: true } },
	afterTest: async (_test, _context, result: { passed: boolean }) => {
		if (!result.passed && artifacts) await browser.saveScreenshot(join(artifacts, `failure-${Date.now()}.png`));
	},
};

if (artifacts) config.outputDir = artifacts;

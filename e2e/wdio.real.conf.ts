import { config as chrome } from "./wdio.fast.conf";
import { LARGE_QUERY_GC_TIME_MS } from "../src/lib/queryKeys";
import { openServerPage } from "./harness/browser-session";

const memory = Boolean(process.env.KUBECOVE_MEMORY_FIXTURE);
const profiling = process.env.KUBECOVE_PROFILE === "1";

function selectSpecs() {
	if (!profiling) return ["./specs/real/**/*.e2e.ts"];
	return memory ? ["./specs/profile/memory.e2e.ts"] : ["./specs/profile/startup.e2e.ts", "./specs/profile/cache-diagnostics.e2e.ts"];
}

export const config: typeof chrome = {
	...chrome,
	baseUrl: process.env.KUBECOVE_SERVER_ORIGIN,
	logLevel: "silent",
	specs: selectSpecs(),
	mochaOpts: { ui: "bdd", timeout: 90_000 + (memory ? LARGE_QUERY_GC_TIME_MS : 0) },
	before: openServerPage,
};

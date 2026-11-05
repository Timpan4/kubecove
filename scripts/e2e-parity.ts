import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { startIdentity, stopGroup } from "../.agents/skills/verify-kubecove/scripts/processes";
import { loadContract, requireViewCoverage, validateReports } from "../e2e/parity/contract";
import { VIEWS } from "../e2e/parity/visual/views";
import { runWeb } from "../e2e/parity/web";

const artifacts = resolve("e2e/artifacts", `parity-${crypto.randomUUID()}`);
await mkdir(artifacts, { recursive: true });
await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: [] }));
await writeFile(join(artifacts, "vite.log"), "Web driver did not start.\n");
await writeFile(join(artifacts, "gpui.log"), "Native driver did not start.\n");
const controller = new AbortController();
const interrupt = () => controller.abort(new Error("Interrupted"));
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
let failure: string | undefined;
try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	// The GPUI driver renders the same browser-mock responses the web driver reads.
	const fixture = join(artifacts, "fixture.json");
	const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
	if (await exported.exited !== 0) throw new Error("Fixture export failed");
	try { await runWeb(contract, artifacts, controller.signal); } catch (error) { failure = `Web driver: ${String(error)}`; }
	controller.signal.throwIfAborted();
	const workspace = resolve("src-tauri/gpui");
	// Keep path-crate artifacts specific to this worktree, including environment overrides.
	const env = { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(workspace, "target"), CARGO_BUILD_BUILD_DIR: process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build"), KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_PARITY_FIXTURE: fixture, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1" };
	delete env.ARGV0; delete env.KUBECONFIG;
	// The GPUI test platform renders without a display.
	const cargo = ["cargo", "test", "--manifest-path", join(workspace, "Cargo.toml"), "--test", "parity", "--", "--nocapture"];
	const command = process.platform === "win32" ? cargo : ["nice", ...cargo];
	const child = Bun.spawn(command, { env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(child.pid);
	const output = Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	const cancel = () => { void stopGroup(child.pid, identity); };
	controller.signal.addEventListener("abort", cancel, { once: true });
	const code = await child.exited;
	controller.signal.removeEventListener("abort", cancel);
	await writeFile(join(artifacts, "gpui.log"), (await output).join("\n"));
	controller.signal.throwIfAborted();
	if (code !== 0) failure = `${failure ? `${failure}; ` : ""}GPUI test exited ${code}`;
	const reports = await Promise.all(["web.json", "gpui.json"].map(async (name) => JSON.parse(await readFile(join(artifacts, name), "utf8"))));
	validateReports(contract, reports);
	if (failure) throw new Error(failure);
} catch (error) { failure = failure ? `${failure}; ${String(error)}` : String(error); }
finally {
	process.removeListener("SIGINT", interrupt); process.removeListener("SIGTERM", interrupt);
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ level: "UI integration with mocks", status: failure ? "failed" : "passed", error: failure }, null, 2));
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }

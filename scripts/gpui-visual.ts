// Captures the same views from the Svelte web frontend and the GPUI client
// with identical browser-mock data, then writes stacked comparisons (web on
// top, GPUI below) and a per-view RMSE score to e2e/artifacts/gpui-visual-<id>/.
// Usage: bun scripts/gpui-visual.ts [--skip-web] [view,view]
// Requires ImageMagick 7 (`magick`).
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { VIEWS } from "../e2e/parity/visual/views";

const args = process.argv.slice(2);
const skipWeb = args.includes("--skip-web");
const views = args.find((arg) => !arg.startsWith("--"))?.split(",") ?? [...VIEWS];
const latest = resolve("e2e/artifacts/gpui-visual");
const out = resolve("e2e/artifacts", `gpui-visual-${new Date().toISOString().replace(/[:.]/g, "-")}`);
await mkdir(join(out, "gpui"), { recursive: true });
await mkdir(join(out, "compare"), { recursive: true });

async function run(command: string[], env: Record<string, string | undefined> = process.env) {
	const child = Bun.spawn(command, { env, stdout: "pipe", stderr: "pipe" });
	const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
	if (code !== 0) throw new Error(`${command.join(" ")} exited ${code}\n${stdout}\n${stderr}`);
	return stdout + stderr;
}

const fixture = join(out, "fixture.json");
await run(["bun", "e2e/parity/visual/export-fixture.ts", fixture]);
// Web references change only with the Svelte app; reuse the last capture when asked.
const webDir = skipWeb ? join(latest, "web") : join(out, "web");
if (!skipWeb) {
    await run(["bun", "e2e/parity/visual/capture-web.ts", webDir], { ...process.env, KUBECOVE_VISUAL_VIEWS: views.join(",") });
	// A fresh capture becomes the reference that --skip-web reuses.
	await cp(webDir, join(latest, "web"), { recursive: true });
}

const workspace = resolve("src-tauri/gpui");
// Keep both Cargo directories specific to this worktree. Shared path-crate
// artifacts can overwrite each other across checkouts. The job count comes from
// the machine's cargo configuration; set CARGO_BUILD_JOBS on a machine short of RAM.
const env = { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(workspace, "target"), CARGO_BUILD_BUILD_DIR: process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build"), KUBECOVE_VISUAL_FIXTURE: fixture, KUBECOVE_VISUAL_OUT: join(out, "gpui"), KUBECOVE_VISUAL_VIEWS: views.join(",") };
delete env.ARGV0;
delete env.KUBECOVE_E2E_ARTIFACTS;
await writeFile(join(out, "gpui.log"), await run(["nice", "cargo", "test", "--locked", "--manifest-path", join(workspace, "Cargo.toml"), "--test", "visual"], env));

const scores: Record<string, number> = {};
for (const view of views) {
	const web = join(webDir, `${view}.png`);
	const gpui = join(out, "gpui", `${view}.png`);
	await run(["magick", web, gpui, "-append", join(out, "compare", `${view}.png`)]);
	// `compare` exits 1 when images differ; read the metric from its output.
	const child = Bun.spawn(["magick", "compare", "-metric", "RMSE", web, gpui, join(out, "compare", `${view}-diff.png`)], { stdout: "pipe", stderr: "pipe" });
	const [code, stderr] = await Promise.all([child.exited, new Response(child.stderr).text()]);
	if (code > 1) throw new Error(`magick compare failed for ${view}: ${stderr}`);
	scores[view] = Number(/\(([\d.e-]+)\)/.exec(stderr)?.[1] ?? Number.NaN);
}
await writeFile(join(out, "scores.json"), JSON.stringify(scores, null, 2));
const previous = await readFile(join(latest, "scores.json"), "utf8").then(JSON.parse, () => ({}));
for (const view of views) {
	const before = previous[view] === undefined ? "" : ` (previous ${previous[view].toFixed(4)})`;
	console.log(`${view}: RMSE ${scores[view].toFixed(4)}${before}`);
}
await mkdir(latest, { recursive: true });
await writeFile(join(latest, "scores.json"), JSON.stringify({ ...previous, ...scores }, null, 2));
console.log(`Visual artifacts: ${out}`);

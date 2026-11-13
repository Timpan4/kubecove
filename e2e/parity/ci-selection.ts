import { appendFile, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { changedScenarios, type Selection } from "./changed-scenarios";
import { loadContract } from "./contract";
import { count, object, string } from "./contract-values";
import { readMap } from "./coverage-map";
import { selectedContract } from "./selected-contract";

export const SPIKE = "settings-diagnostics.topology-spike-harness";
// Full-suite counts from parity.yml before selection.
const FULL_SHARDS = { web: 4, gpui: 3 };

export function selectionPlan(ids: string[], selection: Selection) {
	selectedContract({ scenarios: ids.map((id) => ({ id, title: id, steps: [] })) }, JSON.stringify([...selection.ids]));
	const selected = ids.filter((id) => selection.ids.has(id));
	const regularCount = selected.filter((id) => id !== SPIKE).length;
	const regular = regularCount > 0;
	// A shard of a partial selection carries at most the scenarios a full-suite shard carries,
	// so a large selection takes no longer per job than the full suite does.
	const shards = (driver: keyof typeof FULL_SHARDS) => {
		if (selected.length === ids.length) return FULL_SHARDS[driver];
		const perShard = Math.ceil(ids.filter((id) => id !== SPIKE).length / FULL_SHARDS[driver]);
		return regular ? Math.ceil(regularCount / perShard) : 0;
	};
	return {
		ids: selected, total: ids.length,
		reason: selection.all ?? (selected.length ? "Changed source coverage and scenarios without recorded coverage" : "No scenario covers the changed code"),
		fallback: selection.all ?? null, because: selection.because,
		regular, spike: selected.includes(SPIKE),
		webShards: shards("web"), gpuiShards: shards("gpui"),
		// Keep a valid placeholder matrix. The job-level if prevents it from starting
		// when no regular scenario was selected.
		webMatrix: { shard: Array.from({ length: shards("web") || 1 }, (_, index) => index) },
		gpuiMatrix: { shard: Array.from({ length: shards("gpui") || 1 }, (_, index) => index) },
	};
}

async function command(args: string[]): Promise<string> {
	const child = Bun.spawn(args, { stdout: "pipe", stderr: "pipe" });
	const [stdout] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	if (await child.exited !== 0) throw new Error(`${args.slice(0, 3).join(" ")} failed`);
	return stdout.trim();
}

/** Only a successful main push supplies the map. Read-only fork tokens use the same path. */
async function downloadMap(): Promise<string> {
	const repo = string(process.env.GITHUB_REPOSITORY);
	const runs: unknown = JSON.parse(await command(["gh", "run", "list", "--repo", repo, "--workflow", "parity.yml", "--branch", "main", "--event", "push", "--status", "success", "--limit", "1", "--json", "databaseId,headSha"]));
	if (!Array.isArray(runs) || !runs.length) throw new Error("No successful Parity run on main");
	const run = object(runs[0]);
	const runId = count(run.databaseId);
	if (!runId) throw new Error("Invalid main run ID");
	const directory = "e2e/artifacts/pr-selection-coverage";
	await command(["gh", "run", "download", String(runId), "--repo", repo, "--name", "parity-coverage-map", "--dir", directory]);
	const path = join(directory, "parity-coverage-map.json");
	const map = await readMap(path);
	if (map.commit !== string(run.headSha)) throw new Error("Coverage map does not match its main run");
	console.log(`Using parity-coverage-map from main run ${runId}, commit ${map.commit}`);
	// Full checkout history normally contains it. An old successful run may predate the
	// fetched history or main may have moved; fetch the recorded SHA if it is absent.
	if (!/^[a-f0-9]{40}$/.test(map.commit)) throw new Error("Invalid coverage map commit");
	try { await command(["git", "cat-file", "-e", `${map.commit}^{commit}`]); }
	catch { await command(["git", "fetch", "--no-tags", "origin", map.commit]); }
	return path;
}

/** Any acquisition, history or selection failure selects the full contract. */
export async function planPullRequest(ids: string[], head: string, acquireMap = downloadMap) {
	try {
		const mapPath = await acquireMap();
		const map = await readMap(mapPath);
		// The map describes the tree at its commit, so the changes that can outdate it are those
		// from that commit to the tested head: the pull request's and main's since the map. The
		// pull request's merge base is not used. Diffing from a merge base older than the map
		// would also select main changes the map already recorded.
		await command(["git", "merge-base", "--is-ancestor", map.commit, head]);
		return selectionPlan(ids, await changedScenarios(map.commit, ids, mapPath, head));
	} catch (error) {
		return selectionPlan(ids, { all: `Selection unavailable: ${error instanceof Error ? error.message : String(error)}`, ids: new Set(ids), because: {} });
	}
}

if (import.meta.main) {
	const ids = (await loadContract()).scenarios.map(({ id }) => id);
	const plan = await planPullRequest(ids, "HEAD");
	await mkdir("e2e/artifacts", { recursive: true });
	await writeFile("e2e/artifacts/pr-parity-selection.json", JSON.stringify(plan, null, 2));
	if (process.env.GITHUB_OUTPUT) {
		const outputs = { ids: JSON.stringify(plan.ids), regular: plan.regular, spike: plan.spike, "web-shards": plan.webShards, "gpui-shards": plan.gpuiShards, "web-matrix": JSON.stringify(plan.webMatrix), "gpui-matrix": JSON.stringify(plan.gpuiMatrix), reason: JSON.stringify(plan.reason) };
		await appendFile(process.env.GITHUB_OUTPUT, Object.entries(outputs).map(([name, value]) => `${name}=${value}\n`).join(""));
	}
	const summary = `Parity selected ${plan.ids.length} of ${plan.total} scenarios for both drivers. ${plan.reason}.\n${plan.ids.length ? "" : "No parity scenarios selected; no driver jobs are needed.\n"}`;
	console.log(summary);
	if (process.env.GITHUB_STEP_SUMMARY) await appendFile(process.env.GITHUB_STEP_SUMMARY, summary);
}

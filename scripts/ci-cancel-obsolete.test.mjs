import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { cancelObsoleteRuns } from "./ci-cancel-obsolete.mjs";

// Failure modes: stale main reruns displace current checks, closed PR runs retain
// runners, fork branch names collide, API errors look like deletion, and releases
// share a commit with ordinary validation. Pagination must precede cancellation.
const repo = { owner: "Timpan4", repo: "kubecove" };
const main = "a".repeat(40);
const old = "b".repeat(40);
const run = (id, extra = {}) => ({ id, path: ".github/workflows/ci.yml", event: "push", status: "queued", head_branch: "main", head_sha: old, head_repository: { full_name: "Timpan4/kubecove" }, pull_requests: [], ...extra });

async function exercise(runs, options = {}) {
  const cancelled = [];
  const warnings = [];
  const listed = [];
  const calls = { branch: 0, pull: 0, commits: 0, replacements: 0 };
  const github = {
    rest: {
      git: { getRef: async () => {
        if (options.tag) return { data: { ref: "refs/tags/app-v1" } };
        throw Object.assign(new Error("missing tag"), { status: 404 });
      } },
      repos: { getBranch: async ({ branch }) => {
        calls.branch++;
        if (options.branchError && branch !== "main") throw Object.assign(new Error("lookup failed"), { status: options.branchError });
        return { data: { commit: { sha: main } } };
      } },
      actions: {
        listWorkflowRunsForRepo: "list",
        getWorkflowRun: async ({ run_id }) => ({ data: { ...runs.find((item) => item.id === run_id), ...options.refresh } }),
        cancelWorkflowRun: async ({ run_id }) => {
          assert.deepEqual(listed.toSorted(), ["in_progress", "pending", "queued", "requested", "waiting"]);
          cancelled.push(run_id);
        },
        listJobsForWorkflowRun: "jobs",
      },
      pulls: {
        get: async () => { calls.pull++; if (options.error) throw new Error("unavailable"); return { data: options.pr }; },
        list: "pulls",
        listCommits: "commits",
      },
    },
    paginate: async (method, params) => {
      if (method === "jobs") {
        if (options.jobError) throw new Error("jobs unavailable");
        const path = runs.find((item) => item.id === params.run_id)?.path;
        let name = "check";
        let selection = null;
        if (path === ".github/workflows/ci.yml") {
          name = "CI rust";
          selection = "Detect CI changes";
        } else if (path === ".github/workflows/codspeed.yml") {
          name = "Run frontend benchmarks (0)";
          selection = "Detect benchmark changes";
        }
        const selected = options.jobs?.[params.run_id] ?? [{ name, status: "in_progress", conclusion: null }];
        // nixStep gives CI's "Select Nix build" step a conclusion, or "in_progress".
        const nixStep = options.nixStep?.[params.run_id];
        const steps = nixStep === undefined ? undefined : [
          { name: "Detect relevant changes", status: "completed", conclusion: "success" },
          { name: "Select Nix build", status: nixStep === "in_progress" ? "in_progress" : "completed", conclusion: nixStep === "in_progress" ? null : nixStep },
        ];
        const visible = selection && !selected.some((job) => job.name === selection) ? [{ name: selection, status: "completed", conclusion: "success", steps }, ...selected] : selected;
        if (path === ".github/workflows/ci.yml" && !options.hiddenNix?.includes(params.run_id) && !visible.some((job) => job.name === "Nix build")) {
          return [...visible, { name: "Nix build", status: "completed", conclusion: "skipped" }];
        }
        return visible;
      }
      if (method === "commits") {
        calls.commits++;
        if (options.commitError) throw new Error("commit lookup unavailable");
        return options.commits ?? [];
      }
      if (method === "pulls") { if (options.error) throw Object.assign(new Error("unavailable"), { status: options.error }); return options.prs ?? (options.pr ? [options.pr] : []); }
      assert.equal(method, "list");
      if (params.head_sha) {
        calls.replacements++;
        return runs.filter((item) => item.head_sha === params.head_sha);
      }
      listed.push(params.status);
      return runs.filter((item) => item.status === params.status);
    },
  };
  await cancelObsoleteRuns({ github, context: { repo, payload: { repository: { default_branch: "main", full_name: "Timpan4/kubecove" } } }, core: { info() {}, warning(message) { warnings.push(message); } } });
  return { cancelled, warnings, calls };
}

test("cancels obsolete main validation, including late reruns, and preserves latest main and releases", async () => {
  const result = await exercise([
    run(1), run(2, { head_sha: main }), run(3, { path: ".github/workflows/release.yml" }),
    run(4, { event: "workflow_dispatch", status: "in_progress" }),
    run(5, { head_branch: "app-v1", event: "push" }),
    run(6, { path: ".github/workflows/server-packaging.yml", head_sha: main }),
    run(7, { path: ".github/workflows/release.yml", head_sha: main }),
  ]);
  assert.deepEqual(result.cancelled, [1, 4]);
});

test("cancels closed PR validation across pending states without confusing a fork's main", async () => {
  const pr = { state: "closed", number: 8, head: { sha: old, ref: "topic", repo: { full_name: "fork/kubecove" } } };
  const extra = { event: "pull_request", head_branch: "topic", head_repository: pr.head.repo, pull_requests: [{ number: 8 }] };
  const result = await exercise([run(1, { ...extra, status: "in_progress" }), run(2, { ...extra, status: "pending" }), run(3, { ...extra, status: "waiting" }), run(4, { event: "pull_request", head_repository: { full_name: "fork/kubecove" } })], { pr });
  assert.deepEqual(result.cancelled, [1, 2, 3]);
});

test("handles missing run associations using exact head identity and leaves open or unknown PRs alone", async () => {
  const runs = [run(1, { event: "pull_request", head_branch: "topic" })];
  const pr = { state: "closed", head: { sha: old, ref: "topic", repo: { full_name: "Timpan4/kubecove" } } };
  assert.deepEqual((await exercise(runs, { pr })).cancelled, [1]);
  for (const candidate of [undefined, { ...pr, state: "open" }, { ...pr, head: { ...pr.head, sha: main } }]) {
    assert.deepEqual((await exercise(runs, { pr: candidate })).cancelled, []);
  }
  const failed = await exercise(runs, { error: true });
  assert.deepEqual(failed.cancelled, []);
  assert.equal(failed.warnings.length, 1);
});

test("verifies old commits of closed fork PRs with missing associations without confusing reused branches", async () => {
  const headRepository = { full_name: "fork/kubecove" };
  const runs = [run(1, { event: "pull_request", head_branch: "topic", head_repository: headRepository })];
  const pr = { number: 8, state: "closed", head: { sha: main, ref: "topic", repo: headRepository } };
  assert.deepEqual((await exercise(runs, { pr, commits: [{ sha: old }, { sha: main }] })).cancelled, [1]);
  assert.deepEqual((await exercise(runs, { pr, commits: [{ sha: main }] })).cancelled, []);
  assert.deepEqual((await exercise(runs, { pr, commitError: true })).cancelled, []);
  assert.deepEqual((await exercise(runs, { prs: [pr, { ...pr, state: "open" }], commits: [{ sha: old }] })).cancelled, []);
});

test("E2E primary and matrix runs cannot cancel each other's complementary jobs", () => {
  const workflow = readFileSync(new URL("../.github/workflows/e2e.yml", import.meta.url), "utf8");
  const template = workflow.match(/^ {2}group: (.+)$/m)?.[1];
  assert.equal(template, "${{ github.workflow }}-${{ github.ref }}-${{ github.sha }}-${{ inputs.compatibility-only && 'release-compatibility' || github.event_name == 'push' && 'primary' || 'matrix' }}");
  const jobIf = (name) => {
    const block = workflow.match(new RegExp(`(?:^|\\n) {2}${name}:\\n([\\s\\S]*?)(?=\\n {2}[a-z0-9_-]+:|\\n[a-z0-9_-]+:|$)`))?.[1] ?? "";
    return block.match(/^ {4}if: (.+)$/m)?.[1];
  };
  assert.equal(jobIf("real-primary"), "${{ !inputs.compatibility-only && github.event_name == 'push' }}");
  assert.equal(jobIf("real-matrix"), "${{ inputs.compatibility-only || github.event_name != 'push' }}");
  assert.equal(jobIf("desktop-smoke"), "${{ !inputs.compatibility-only && github.event_name != 'push' }}");
});

test("selective main pushes cannot cancel full manual CI at the same SHA", () => {
  const workflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
  const group = workflow.match(/^ {2}group: (.+)$/m)[1];
  assert.ok(group.endsWith("-${{ github.event_name == 'workflow_dispatch' && 'full' || 'changes' }}"));
});

test("a newer main commit must replace the same work before an old run is cancelled", async () => {
  const first = run(1);
  const latest = run(2, { head_sha: main, status: "completed", conclusion: "success" });
  const rust = { name: "CI rust", status: "in_progress", conclusion: null };
  const docs = { name: "CI docs", status: "completed", conclusion: "success" };
  assert.deepEqual((await exercise([first])).cancelled, [], "no current workflow run");
  assert.deepEqual((await exercise([run(1, { path: ".github/workflows/server-packaging.yml" }), latest])).cancelled, [], "paths filter omitted packaging");
  assert.deepEqual((await exercise([first, latest], { jobs: { 1: [rust], 2: [docs] } })).cancelled, [], "change selection omitted Rust");
  assert.deepEqual((await exercise([first, latest], { jobs: { 1: [{ name: "Detect CI changes", status: "in_progress", conclusion: null }], 2: [docs] } })).cancelled, [], "the original matrix is not known while detection runs");
  for (const replacement of [[], [{ ...rust, status: "completed", conclusion: "skipped" }], [{ ...rust, status: "completed", conclusion: "failure" }]]) {
    assert.deepEqual((await exercise([first, latest], { jobs: { 1: [rust], 2: replacement } })).cancelled, []);
  }
  assert.deepEqual((await exercise([first, latest], { jobs: { 1: [rust], 2: [rust, docs] } })).cancelled, [1]);
  assert.deepEqual((await exercise([first, { ...latest, conclusion: "cancelled" }], { jobs: { 1: [rust], 2: [rust] } })).cancelled, [], "cancelled replacements can still have draining jobs");
  assert.deepEqual((await exercise([first, latest], { jobError: true })).cancelled, []);
  const matrix = run(1, { path: ".github/workflows/e2e.yml", event: "schedule" });
  const primary = run(2, { path: matrix.path, head_sha: main });
  assert.deepEqual((await exercise([matrix, primary])).cancelled, [], "primary cannot replace the matrix");
  assert.deepEqual((await exercise([matrix, { ...primary, event: "workflow_dispatch" }])).cancelled, [1]);
  const benchmark = run(1, { path: ".github/workflows/codspeed.yml" });
  const newerBenchmark = run(2, { path: benchmark.path, head_sha: main });
  assert.deepEqual((await exercise([benchmark, newerBenchmark])).cancelled, [1], "CodSpeed selects its full baseline on main pushes");
});

test("retains CI until its Nix selection is known and replaced", async () => {
  const original = run(1);
  const replacement = run(2, { head_sha: main });
  const frontend = { name: "CI frontend-e2e", status: "in_progress", conclusion: null };
  const nix = { name: "Nix build", status: "queued", conclusion: null };
  const jobs = { 1: [frontend], 2: [frontend] };
  const both = { jobs, hiddenNix: [1, 2] };
  assert.deepEqual((await exercise([original, replacement], { ...both, hiddenNix: [1] })).cancelled, [], "a run without the selection step keeps its unknown Nix selection");
  for (const nixStep of ["in_progress", "failure", "cancelled"]) {
    assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: nixStep, 2: "success" } })).cancelled, [], `an unfinished or failed selection step (${nixStep}) is unknown`);
  }
  assert.deepEqual((await exercise([original, replacement], { jobs: { ...jobs, 1: [frontend, nix] } })).cancelled, [], "frontend-only replacement omits selected Nix");
  assert.deepEqual((await exercise([original, replacement], { jobs: { 1: [frontend, nix], 2: [frontend, nix] } })).cancelled, [1]);
});

// Superseded main CI used to run every matrix task before its Nix job appeared, and only then
// could a newer run replace it. The selection step makes the Nix selection known at once.
test("cancels superseded CI as soon as the newer run selects the same tasks and Nix", async () => {
  const original = run(1);
  const replacement = run(2, { head_sha: main });
  const rust = { name: "CI rust", status: "in_progress", conclusion: null };
  const docs = { name: "CI docs", status: "in_progress", conclusion: null };
  const both = { jobs: { 1: [rust], 2: [rust] }, hiddenNix: [1, 2] };
  assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: "success", 2: "success" } })).cancelled, [1]);
  assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: "skipped", 2: "skipped" } })).cancelled, [1]);
  assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: "skipped", 2: "success" } })).cancelled, [1]);
  assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: "success", 2: "skipped" } })).cancelled, [], "the newer run does not build Nix");
  assert.deepEqual((await exercise([original, replacement], { ...both, nixStep: { 1: "success" } })).cancelled, [], "the newer run's Nix selection is unknown");
  assert.deepEqual((await exercise([original, replacement], { ...both, jobs: { 1: [rust], 2: [docs] }, nixStep: { 1: "skipped", 2: "success" } })).cancelled, [], "the newer run does not test Rust");
  // A Nix job that the newer run already shows counts only while it is active or passed.
  for (const [nix, cancelled] of [[{ status: "queued", conclusion: null }, [1]], [{ status: "completed", conclusion: "success" }, [1]], [{ status: "completed", conclusion: "failure" }, []]]) {
    const jobs = { 1: [rust], 2: [rust, { name: "Nix build", ...nix }] };
    assert.deepEqual((await exercise([original, replacement], { jobs, hiddenNix: [1], nixStep: { 1: "success", 2: "success" } })).cancelled, cancelled, JSON.stringify(nix));
  }
  const detecting = { name: "Detect CI changes", status: "in_progress", conclusion: null };
  assert.deepEqual((await exercise([original, replacement], { jobs: { 1: [detecting], 2: [rust] }, hiddenNix: [1, 2], nixStep: { 1: "skipped", 2: "success" } })).cancelled, [], "the older run's selection is still running");
  const marked = { ...detecting, steps: [{ name: "Select Nix build", status: "completed", conclusion: "success" }] };
  assert.deepEqual((await exercise([original, replacement], { jobs: { 1: [rust], 2: [rust, marked] }, hiddenNix: [1, 2], nixStep: { 1: "success" } })).cancelled, [], "the newer run's Detect job has not published its outputs");
});

// Superseded Parity and CodSpeed main runs used to run their shards until the newer run's
// build finished and its shards appeared.
test("a newer main Parity or CodSpeed run replaces every job of an older one at once", async () => {
  for (const path of [".github/workflows/parity.yml", ".github/workflows/codspeed.yml"]) {
    const shards = path.endsWith("parity.yml") ? [{ name: "Parity (main) / Both shard 0 of 4 ", status: "in_progress", conclusion: null }] : [{ name: "Run Rust benchmarks (dl)", status: "in_progress", conclusion: null }];
    const building = [{ name: path.endsWith("parity.yml") ? "Build instrumented parity GPUI (main) / GPUI build (artifact)" : "Detect benchmark changes", status: "in_progress", conclusion: null }];
    const original = run(1, { path });
    const newer = run(2, { path, head_sha: main, status: "in_progress" });
    const jobs = { 1: shards, 2: building };
    assert.deepEqual((await exercise([original, newer], { jobs })).cancelled, [1], path);
    assert.deepEqual((await exercise([original], { jobs })).cancelled, [], `${path}: no newer run`);
    for (const conclusion of ["failure", "cancelled"]) {
      assert.deepEqual((await exercise([original, { ...newer, status: "completed", conclusion }], { jobs })).cancelled, [], `${path}: ${conclusion} newer run`);
    }
    assert.deepEqual((await exercise([original, { ...newer, status: "completed", conclusion: "success" }], { jobs })).cancelled, [1]);
    assert.deepEqual((await exercise([original, { ...newer, head_sha: old }], { jobs })).cancelled, [], `${path}: a run at the old commit`);
    assert.deepEqual((await exercise([original, { ...newer, head_branch: "topic" }], { jobs })).cancelled, [], `${path}: a branch run`);
  }
});

// Failure modes: the selection step's condition drifts from the Nix job's, so a run that builds
// Nix reports that it does not; a renamed step or job leaves every selection unknown or wrong;
// or Parity or CodSpeed gain push-side selection, so a newer run no longer replaces every job.
test("the cancellation rules match the workflow graphs they rely on", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const jobsOf = (text) => [...text.slice(text.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):\n((?: {4}.*\n|\n)*)/gm)].map(([, id, body]) => ({ id, body }));
  const ci = Object.fromEntries(jobsOf(read("ci.yml")).map(({ id, body }) => [id, body]));
  assert.match(ci.changes, /^ {4}name: Detect CI changes$/m);
  assert.match(ci.changes, /^ {6}nix: \$\{\{ steps\.detect\.outputs\.nix \}\}$/m);
  const marker = /^ {6}- name: Select Nix build\n {8}if: \$\{\{ (.+) \}\}$/m.exec(ci.changes)?.[1];
  assert.ok(marker?.includes("steps.detect.outputs.nix == 'true'"));
  assert.match(ci.nix, /^ {4}name: Nix build$/m);
  assert.equal(/^ {4}if: \$\{\{ (.+) \}\}$/m.exec(ci.nix)?.[1], `always() && !cancelled() && ${marker.replace("steps.detect.outputs.nix", "needs.changes.outputs.nix")}`);
  // Every Parity job that can run on a push decides by event and branch, with a fixed matrix.
  const pushJobs = jobsOf(read("parity.yml")).filter(({ body }) => !/^ {4}if: .*github\.event_name == 'pull_request'/m.test(body));
  assert.deepEqual(pushJobs.map(({ id }) => id), ["timings", "main-gpui-build", "main", "main-spike", "main-build-cache", "publish-timings", "coverage-map"]);
  for (const { id, body } of pushJobs) {
    assert.doesNotMatch(/^ {4}if: (.+)$/m.exec(body)?.[1] ?? "", /outputs|result/, id);
    assert.doesNotMatch(body, /fromJSON/, id);
  }
  // CodSpeed's detection gets the event and selects every benchmark outside pull requests.
  assert.match(read("codspeed.yml"), /^ {10}EVENT_NAME: \$\{\{ github\.event_name \}\}$/m);
  const root = mkdtempSync(join(tmpdir(), "kubecove-codspeed-full-"));
  try {
    for (const event of ["push", "workflow_dispatch"]) {
      const output = join(root, event);
      writeFileSync(output, "");
      const result = spawnSync(process.execPath, [fileURLToPath(new URL("./codspeed-detect.mjs", import.meta.url)), join(root, "unused")], {
        env: { ...process.env, EVENT_NAME: event, GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: "" }, encoding: "utf8",
      });
      assert.equal(result.status, 0, result.stderr);
      assert.match(readFileSync(output, "utf8"), /^frontend=true\nrust=true\n/, event);
      assert.match(result.stdout + readFileSync(output, "utf8"), /frontend_matrix=/);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("tag dispatches survive a same-name default or merged branch", async () => {
  const pr = { state: "closed", head: { sha: old, ref: "topic", repo: { full_name: "Timpan4/kubecove" } } };
  for (const head_branch of ["main", "topic"]) {
    const runs = [run(1, { event: "workflow_dispatch", head_branch }), run(2, { head_sha: main })];
    assert.deepEqual((await exercise(runs, { pr, tag: true })).cancelled, []);
  }
});

test("a backlog shares main and PR identity reads within one reconciliation", async () => {
  const backlog = activeRuns();
  const result = await exercise([...backlog, run(99, { head_sha: main })]);
  assert.equal(result.calls.branch, 1);
  assert.equal(result.calls.replacements, 1);
  const pr = { number: 8, state: "closed", head: { sha: old, ref: "topic", repo: { full_name: "Timpan4/kubecove" } } };
  const resultPr = await exercise(backlog.map((item) => ({ ...item, event: "pull_request", head_branch: "topic", pull_requests: [{ number: 8 }] })), { pr });
  assert.equal(resultPr.calls.pull, 1);
});

function activeRuns() {
  return ["queued", "in_progress", "pending", "waiting", "requested"].map((status, index) => run(index + 1, { status }));
}

test("cleanup uses the existing API cleanup job deadline", () => {
  const cleanup = readFileSync(new URL("../.github/workflows/ci-cancel-obsolete.yml", import.meta.url), "utf8");
  const cacheCleanup = readFileSync(new URL("../.github/workflows/rust-cache-cleanup.yml", import.meta.url), "utf8");
  assert.equal(cleanup.match(/timeout-minutes: (\d+)/)?.[1], cacheCleanup.match(/timeout-minutes: (\d+)/)[1]);
});

test("cancels deleted branch dispatches only after a confirmed missing branch", async () => {
  const runs = [run(1, { event: "workflow_dispatch", head_branch: "deleted-topic" })];
  assert.deepEqual((await exercise(runs, { branchError: 404 })).cancelled, [1]);
  for (const branchError of [undefined, 403, 500]) assert.deepEqual((await exercise(runs, { branchError })).cancelled, []);
  assert.deepEqual((await exercise(runs, { branchError: 404, tag: true })).cancelled, [], "manual validation on an existing tag is retained");
  const pr = { state: "closed", head: { sha: old, ref: "deleted-topic", repo: { full_name: "Timpan4/kubecove" } } };
  assert.deepEqual((await exercise(runs, { pr })).cancelled, [1], "merged branch dispatches are obsolete even before branch deletion");
  assert.deepEqual((await exercise(runs, { error: 404 })).cancelled, [], "a missing PR lookup must not mean the existing branch was deleted");
});

test("refreshes runs after listing and handles closed PRs whose last push superseded the run", async () => {
  assert.deepEqual((await exercise([run(1)], { refresh: { status: "completed" } })).cancelled, []);
  const pr = { state: "closed", head: { sha: main } };
  assert.deepEqual((await exercise([run(1, { event: "pull_request", head_branch: "topic", pull_requests: [{ number: 8 }] })], { pr })).cancelled, [1]);
});

test("validation concurrency replaces older work and isolates reusable release packaging", () => {
  for (const file of ["ci", "parity", "codspeed", "tsrs", "e2e"]) {
    const text = readFileSync(new URL(`../.github/workflows/${file}.yml`, import.meta.url), "utf8");
    assert.match(text, /cancel-in-progress: true/);
    if (file === "e2e") assert.match(text, /group:.*-\$\{\{ github\.sha \}\}-/);
    else assert.ok(text.includes("${{ github.event_name != 'pull_request' && github.sha || '' }}"), `${file} must isolate every non-PR commit`);
  }
  const packaging = readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8");
  assert.match(packaging, /inputs\.release-tag \|\| github\.ref/);
  assert.match(packaging, /cancel-in-progress:.*!inputs\.release-tag/);
  assert.ok(packaging.includes("${{ !inputs.release-tag && github.sha || '' }}"));
  const cleanup = readFileSync(new URL("../.github/workflows/ci-cancel-obsolete.yml", import.meta.url), "utf8");
  assert.match(cleanup, /pull_request_target:\n {4}types: \[closed\]/);
  assert.match(cleanup, / {2}delete:/);
  assert.match(cleanup, /types: \[in_progress, completed\]/);
  assert.match(cleanup, /ref: \$\{\{ github\.event\.repository\.default_branch \}\}/);
  assert.doesNotMatch(cleanup, /head\.sha|head_ref|download-artifact/);
});

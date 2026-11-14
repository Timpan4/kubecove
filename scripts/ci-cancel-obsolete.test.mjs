import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
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
  const github = {
    rest: {
      git: { getRef: async () => {
        if (options.tag) return { data: { ref: "refs/tags/app-v1" } };
        throw Object.assign(new Error("missing tag"), { status: 404 });
      } },
      repos: { getBranch: async ({ branch }) => {
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
      },
      pulls: {
        get: async () => { if (options.error) throw new Error("unavailable"); return { data: options.pr }; },
        list: "pulls",
      },
    },
    paginate: async (method, params) => {
      if (method === "pulls") { if (options.error) throw Object.assign(new Error("unavailable"), { status: options.error }); return options.pr ? [options.pr] : []; }
      assert.equal(method, "list");
      listed.push(params.status);
      return runs.filter((item) => item.status === params.status);
    },
  };
  await cancelObsoleteRuns({ github, context: { repo, payload: { repository: { default_branch: "main", full_name: "Timpan4/kubecove" } } }, core: { info() {}, warning(message) { warnings.push(message); } } });
  return { cancelled, warnings };
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
    assert.match(text, /github\.ref == 'refs\/heads\/main' && github\.sha/);
  }
  const packaging = readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8");
  assert.match(packaging, /inputs\.release-tag \|\| github\.ref/);
  assert.match(packaging, /cancel-in-progress:.*!inputs\.release-tag/);
  const cleanup = readFileSync(new URL("../.github/workflows/ci-cancel-obsolete.yml", import.meta.url), "utf8");
  assert.match(cleanup, /pull_request_target:\n {4}types: \[closed\]/);
  assert.match(cleanup, / {2}delete:/);
  assert.match(cleanup, /types: \[in_progress\]/);
  assert.match(cleanup, /ref: \$\{\{ github\.event\.repository\.default_branch \}\}/);
  assert.doesNotMatch(cleanup, /head\.sha|head_ref|download-artifact/);
});

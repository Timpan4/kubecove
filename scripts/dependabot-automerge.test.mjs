import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { findPullRequest, followedWorkflows, mergeReadiness, settlePullRequest } from "./dependabot-automerge.mjs";

// Failure modes: a stale event acts on a newer head; a human, fork, retargeted or non-main pull
// request merges; no reported checks reads as all passed; a failed required check or a pending
// advisory check (Parity) still arms auto-merge; a later push keeps an earlier arming; a stale or
// missing bun.nix merges; a removed bun.lock is regenerated; GitHub's "clean status" refusal
// leaves a ready pull request unmerged; a missing token fails after a partial write; a second
// page of checks is ignored; a workflow that has not started, or whose later jobs have not
// queued, reads as finished; the job runs pull request code.
// CI runs this suite before installing packages, so it imports only Node built-ins.
const repo = { owner: "Timpan4", repo: "kubecove" };
const repository = { full_name: "Timpan4/kubecove", default_branch: "main" };
const head = "a".repeat(40);
const newer = "b".repeat(40);

const check = (name, status, conclusion, isRequired = false) => ({ __typename: "CheckRun", name, status, conclusion, isRequired });
const status = (context, state, isRequired = false) => ({ __typename: "StatusContext", context, state, isRequired });

const completedRuns = ["CI", "Parity", "CodSpeed"].map((name) => ({ name, status: "completed" }));

function harness({ pull = {}, pages = [[]], files = {}, armError, runs = completedRuns } = {}) {
  const writes = [];
  const notices = [];
  const pr = {
    number: 7, state: "open", node_id: "PR_1", auto_merge: null, user: { login: "dependabot[bot]" }, base: { ref: "main" },
    head: { sha: head, ref: "dependabot/bun/x", repo: { full_name: "Timpan4/kubecove" } }, ...pull,
  };
  const github = {
    paginate: async (method, params) => {
      assert.equal(method, "runs");
      assert.deepEqual(params, { ...repo, head_sha: head, event: "pull_request" });
      return runs;
    },
    rest: {
      actions: { listWorkflowRunsForRepo: "runs" },
      pulls: { get: async () => ({ data: pr }) },
      repos: { getContent: async ({ path, ref }) => {
        assert.equal(ref, head);
        if (files[path] instanceof Error) throw files[path];
        return { data: files[path] };
      } },
    },
    graphql: async (_query, { cursor }) => {
      const index = cursor ? Number(cursor) : 0;
      return { repository: { pullRequest: { commits: { nodes: [{ commit: { oid: head, statusCheckRollup: { contexts: {
        nodes: pages[index], pageInfo: { hasNextPage: index + 1 < pages.length, endCursor: String(index + 1) },
      } } } }] } } } };
    },
  };
  const writer = {
    graphql: async (query, variables) => {
      const name = query.match(/(createCommitOnBranch|disablePullRequestAutoMerge|enablePullRequestAutoMerge|mergePullRequest)/)[1];
      writes.push({ name, ...variables });
      if (name === "enablePullRequestAutoMerge" && armError) throw new Error(armError);
      if (name === "createCommitOnBranch") return { createCommitOnBranch: { commit: { oid: newer, signature: { isValid: true } } } };
      return {};
    },
  };
  const core = { info() {}, notice: (message) => notices.push(message) };
  const settle = (extra = {}) => settlePullRequest({ github, writer, context: { repo, payload: { repository } }, core, pr: { number: 7, sha: head }, ...extra });
  return { settle, writes, notices };
}

test("readiness waits for every advisory check and refuses any failure", () => {
  const states = (nodes) => nodes.map((node) => ({ name: node.name, state: node.state, required: node.required ?? false }));
  assert.equal(mergeReadiness([], completedRuns).action, "wait");
  assert.equal(mergeReadiness(states([{ name: "Parity result", state: "pending" }, { name: "Typecheck and test", state: "pass", required: true }]), completedRuns).action, "wait");
  assert.equal(mergeReadiness(states([{ name: "CodSpeed Performance Analysis", state: "pending", required: true }, { name: "Parity result", state: "pass" }]), completedRuns).action, "arm");
  assert.equal(mergeReadiness(states([{ name: "Typecheck and test", state: "fail", required: true }, { name: "Parity result", state: "pending" }]), completedRuns).action, "block");
});

test("a followed workflow still running holds the pull request before its later jobs report", async () => {
  // Parity's build passed, but its shards and result job have not queued yet.
  const runs = [{ name: "CI", status: "completed" }, { name: "Parity", status: "in_progress" }];
  const pages = [[check("Typecheck and test", "COMPLETED", "SUCCESS", true), check("Parity build", "COMPLETED", "SUCCESS")]];
  const waiting = harness({ pages, runs });
  assert.equal(await waiting.settle(), "wait");
  assert.deepEqual(waiting.writes, []);
  const armed = harness({ pages, runs, pull: { auto_merge: { enabled_by: { login: "Timpan4" } } } });
  assert.equal(await armed.settle(), "disarmed");
  const unrelated = harness({ pages, runs: [...completedRuns, { name: "E2E", status: "in_progress" }] });
  assert.equal(await unrelated.settle(), "armed");
  // CI and CodSpeed finished before the runs API lists Parity.
  const unlisted = harness({ pages, runs: completedRuns.filter((run) => run.name !== "Parity") });
  assert.equal(await unlisted.settle(), "wait");
  assert.deepEqual(unlisted.writes, []);
});

test("every non-passing conclusion and status blocks, neutral and skipped pass", async () => {
  for (const conclusion of ["FAILURE", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE", "STALE"]) {
    const { settle, writes } = harness({ pages: [[check("Parity result", "COMPLETED", conclusion)]] });
    assert.equal(await settle(), "block", conclusion);
    assert.deepEqual(writes, []);
  }
  for (const state of ["ERROR", "FAILURE"]) {
    const { settle } = harness({ pages: [[status("ci/external", state)]] });
    assert.equal(await settle(), "block", state);
  }
  const { settle } = harness({ pages: [[check("a", "COMPLETED", "NEUTRAL"), check("b", "COMPLETED", "SKIPPED"), status("c", "SUCCESS"), status("d", "PENDING", true)]] });
  assert.equal(await settle(), "armed");
});

test("checks on a later page hold the pull request", async () => {
  const { settle, writes } = harness({ pages: [[check("CI rust", "COMPLETED", "SUCCESS")], [check("Parity result", "IN_PROGRESS", null)]] });
  assert.equal(await settle(), "wait");
  assert.deepEqual(writes, []);
});

test("arming pins the verified head, and a pull request that can already merge merges at that head", async () => {
  const ready = [[check("Parity result", "COMPLETED", "SUCCESS"), check("Typecheck and test", "QUEUED", null, true)]];
  const armed = harness({ pages: ready });
  assert.equal(await armed.settle(), "armed");
  assert.deepEqual(armed.writes, [{ name: "enablePullRequestAutoMerge", id: "PR_1", sha: head }]);

  const clean = harness({ pages: ready, armError: "Pull request Pull request is in clean status" });
  assert.equal(await clean.settle(), "merged");
  assert.deepEqual(clean.writes.map(({ name, sha }) => [name, sha]), [["enablePullRequestAutoMerge", head], ["mergePullRequest", head]]);

  const refused = harness({ pages: ready, armError: "Pull request is in draft state" });
  await assert.rejects(refused.settle(), /draft state/);
});

test("an armed pull request is disarmed when a check restarts or fails, and left alone when still ready", async () => {
  const armedPull = { auto_merge: { enabled_by: { login: "Timpan4" } } };
  const restarted = harness({ pull: armedPull, pages: [[check("Parity result", "QUEUED", null)]] });
  assert.equal(await restarted.settle(), "disarmed");
  assert.deepEqual(restarted.writes, [{ name: "disablePullRequestAutoMerge", id: "PR_1" }]);

  const failed = harness({ pull: armedPull, pages: [[check("CI rust", "COMPLETED", "FAILURE")]] });
  assert.equal(await failed.settle(), "disarmed");

  const ready = harness({ pull: armedPull, pages: [[check("Parity result", "COMPLETED", "SUCCESS")]] });
  assert.equal(await ready.settle(), "armed");
  assert.deepEqual(ready.writes, []);
});

test("a stale bun.nix is committed to the pull request head instead of merging", async () => {
  const pages = [[check("Parity result", "COMPLETED", "SUCCESS")]];
  const stale = harness({ pages, files: { "bun.nix": "old" } });
  assert.equal(await stale.settle({ regeneratedBunNix: "new" }), "committed");
  assert.equal(stale.writes.length, 1);
  const { name, input } = stale.writes[0];
  assert.equal(name, "createCommitOnBranch");
  assert.equal(input.expectedHeadOid, head);
  assert.equal(input.branch.branchName, "dependabot/bun/x");
  assert.deepEqual(input.fileChanges.additions, [{ path: "bun.nix", contents: Buffer.from("new").toString("base64") }]);

  const fresh = harness({ pages, files: { "bun.nix": "same" } });
  assert.equal(await fresh.settle({ regeneratedBunNix: "same" }), "armed");

  const armed = harness({ pages, files: { "bun.nix": "old" }, pull: { auto_merge: { enabled_by: { login: "Timpan4" } } } });
  assert.equal(await armed.settle({ regeneratedBunNix: "new" }), "committed");
  assert.deepEqual(armed.writes.map(({ name }) => name), ["disablePullRequestAutoMerge", "createCommitOnBranch"]);

  const missing = harness({ pages, files: { "bun.nix": Object.assign(new Error("Not Found"), { status: 404 }) } });
  assert.equal(await missing.settle({ regeneratedBunNix: "new" }), "committed");
  const unreadable = harness({ pages, files: { "bun.nix": Object.assign(new Error("Server Error"), { status: 502 }) } });
  await assert.rejects(unreadable.settle({ regeneratedBunNix: "new" }), /Server Error/);
  assert.deepEqual(unreadable.writes, []);
});

test("a moved, closed, retargeted or unauthorized pull request gets no writes", async () => {
  const sameRepo = { full_name: "Timpan4/kubecove" };
  for (const pull of [
    { head: { sha: newer, ref: "dependabot/bun/x", repo: sameRepo } },
    { state: "closed" },
    { base: { ref: "release" } },
    { user: { login: "Timpan4" } },
    { head: { sha: head, ref: "dependabot/bun/x", repo: { full_name: "someone/fork" } } },
  ]) {
    const { settle, writes } = harness({ pull, pages: [[check("Parity result", "COMPLETED", "SUCCESS")]] });
    assert.equal(await settle(), "stale");
    assert.deepEqual(writes, []);
  }
  const { settle } = harness();
  await assert.rejects(settle({ writer: null }), /NIX_LOCK_REFRESH_TOKEN is unavailable/);
});

test("only an open Dependabot pull request into main at the run's head is settled", async () => {
  const run = { event: "pull_request", head_branch: "dependabot/bun/x", head_sha: head, head_repository: { full_name: "Timpan4/kubecove" } };
  const pull = { number: 7, state: "open", head: { sha: head, repo: { full_name: "Timpan4/kubecove" } }, user: { login: "dependabot[bot]" }, base: { ref: "main" } };
  async function find(runChanges = {}, pullChanges = {}, files = [{ filename: "bun.lock" }]) {
    const outputs = {};
    const github = {
      rest: { pulls: { list: "list", listFiles: "files" } },
      paginate: async (method, params) => {
        if (method === "list") {
          assert.equal(params.head, "Timpan4:dependabot/bun/x");
          return [{ ...pull, ...pullChanges }];
        }
        return files;
      },
    };
    const context = { repo, payload: { workflow_run: { ...run, ...runChanges }, repository } };
    const found = await findPullRequest({ github, context, core: { info() {}, setOutput: (key, value) => { outputs[key] = value; } } });
    return { found, outputs };
  }
  const settled = await find();
  assert.deepEqual(settled.found, { number: 7, sha: head, bunLock: true });
  assert.deepEqual(settled.outputs, { number: "7", sha: head, "bun-lock": "true" });
  assert.equal((await find({}, {}, [{ filename: "rust/Cargo.lock", status: "modified" }])).found.bunLock, false);
  assert.equal((await find({}, {}, [{ filename: "bun.lock", status: "removed" }])).found.bunLock, false);
  for (const [runChanges, pullChanges] of [
    [{ head_sha: newer }, {}],
    [{}, { user: { login: "Timpan4" } }],
    [{}, { base: { ref: "release" } }],
    [{ event: "push" }, {}],
    [{ head_repository: { full_name: "someone/fork" } }, {}],
  ]) {
    const { found, outputs } = await find(runChanges, pullChanges);
    assert.equal(found, null);
    assert.deepEqual(outputs, { number: "" });
  }
});

test("the privileged job checks out only the default branch and follows existing workflows", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}.yml`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const workflow = read("dependabot-automerge");
  assert.match(workflow, /^permissions: \{\}$/m);
  assert.match(workflow, /\n {4}permissions:\n {6}actions: read\n {6}checks: read\n {6}contents: read\n {6}pull-requests: read\n {6}statuses: read\n\n/);
  assert.equal(workflow.match(/^ {4}permissions:/gm).length, 1);
  assert.deepEqual(workflow.match(/^ {4}workflows: \[(.+)\]$/m)[1].split(", "), followedWorkflows);
  // Every step starts with "- " at the steps indentation, whatever key comes first.
  const steps = workflow.split(/\n {6}- /).slice(1);
  const checkouts = steps.filter((step) => step.includes("actions/checkout@"));
  assert.equal(checkouts.length, 1);
  assert.match(checkouts[0], /^name: .+\n {8}uses: actions\/checkout@\S+ # v[\d.]+\n {8}with:\n {10}ref: \$\{\{ github\.event\.repository\.default_branch \}\}\n {10}persist-credentials: false\n*$/);
  assert.deepEqual(steps.filter((step) => step.includes("secrets.")).map((step) => step.split("\n")[0]), ["name: Commit bun.nix or update auto-merge"]);
  for (const name of followedWorkflows) {
    const source = read({ CI: "ci", Parity: "parity", CodSpeed: "codspeed" }[name]);
    assert.match(source, new RegExp(`^name: ${name}$`, "m"));
    assert.match(source, /^on:\n(?: {2}\S.*\n(?: {4}.*\n)*)*? {2}pull_request:\n {4}types: \[opened, synchronize, reopened\]\n/m, `${name} must run on every pull request push`);
  }
});

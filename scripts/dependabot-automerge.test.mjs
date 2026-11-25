import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parse } from "yaml";
import { findPullRequest, mergeReadiness, settlePullRequest } from "./dependabot-automerge.mjs";

// Failure modes: a stale event acts on a newer head; a human, fork or non-main pull request
// merges; no reported checks reads as all passed; a failed required check or a pending advisory
// check (Parity) still arms auto-merge; a later push keeps an earlier arming; a stale bun.nix
// merges; GitHub's "clean status" refusal leaves a ready pull request unmerged; a missing token
// fails after a partial write; a second page of checks is ignored; the job runs pull request code.
const repo = { owner: "Timpan4", repo: "kubecove" };
const head = "a".repeat(40);
const newer = "b".repeat(40);

const check = (name, status, conclusion, isRequired = false) => ({ __typename: "CheckRun", name, status, conclusion, isRequired });
const status = (context, state, isRequired = false) => ({ __typename: "StatusContext", context, state, isRequired });

function harness({ pull = {}, pages = [[]], files = {}, armError } = {}) {
  const writes = [];
  const notices = [];
  const pr = { state: "open", head: { sha: head, ref: "dependabot/bun/x" }, node_id: "PR_1", auto_merge: null, ...pull };
  const github = {
    rest: {
      pulls: { get: async () => ({ data: pr }) },
      repos: { getContent: async ({ path, ref }) => {
        assert.equal(ref, head);
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
  const settle = (extra = {}) => settlePullRequest({ github, writer, context: { repo }, core, pr: { number: 7, sha: head }, ...extra });
  return { settle, writes, notices };
}

test("readiness waits for every advisory check and refuses any failure", () => {
  const states = (nodes) => nodes.map((node) => ({ name: node.name, state: node.state, required: node.required ?? false }));
  assert.equal(mergeReadiness([]).action, "wait");
  assert.equal(mergeReadiness(states([{ name: "Parity result", state: "pending" }, { name: "Typecheck and test", state: "pass", required: true }])).action, "wait");
  assert.equal(mergeReadiness(states([{ name: "CodSpeed Performance Analysis", state: "pending", required: true }, { name: "Parity result", state: "pass" }])).action, "arm");
  assert.equal(mergeReadiness(states([{ name: "Typecheck and test", state: "fail", required: true }, { name: "Parity result", state: "pending" }])).action, "block");
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
});

test("a moved, closed or unauthorized run writes nothing", async () => {
  for (const pull of [{ head: { sha: newer, ref: "dependabot/bun/x" } }, { state: "closed" }]) {
    const { settle, writes } = harness({ pull, pages: [[check("Parity result", "COMPLETED", "SUCCESS")]] });
    assert.equal(await settle(), "stale");
    assert.deepEqual(writes, []);
  }
  const { settle } = harness();
  await assert.rejects(settle({ writer: null }), /NIX_LOCK_REFRESH_TOKEN is unavailable/);
});

test("only an open Dependabot pull request into main at the run's head is settled", async () => {
  const run = { event: "pull_request", head_branch: "dependabot/bun/x", head_sha: head, head_repository: { full_name: "Timpan4/kubecove" } };
  const pull = { number: 7, head: { sha: head }, user: { login: "dependabot[bot]" }, base: { ref: "main" } };
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
    const context = { repo, payload: { workflow_run: { ...run, ...runChanges }, repository: { full_name: "Timpan4/kubecove", default_branch: "main" } } };
    const found = await findPullRequest({ github, context, core: { info() {}, setOutput: (key, value) => { outputs[key] = value; } } });
    return { found, outputs };
  }
  const settled = await find();
  assert.deepEqual(settled.found, { number: 7, sha: head, bunLock: true });
  assert.deepEqual(settled.outputs, { number: "7", sha: head, "bun-lock": "true" });
  assert.equal((await find({}, {}, [{ filename: "rust/Cargo.lock" }])).found.bunLock, false);
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
  const workflow = parse(readFileSync(new URL("../.github/workflows/dependabot-automerge.yml", import.meta.url), "utf8"));
  assert.deepEqual(workflow.permissions, {});
  const job = workflow.jobs.settle;
  assert.deepEqual(job.permissions, { checks: "read", contents: "read", "pull-requests": "read", statuses: "read" });
  const checkout = job.steps.find((step) => step.uses?.startsWith("actions/checkout@"));
  assert.match(checkout.with.ref, /^\$\{\{ github\.event\.repository\.default_branch \}\}$/);
  assert.equal(checkout.with["persist-credentials"], false);
  assert.equal(job.steps.filter((step) => step.uses?.startsWith("actions/checkout@")).length, 1);
  const secretSteps = job.steps.filter((step) => JSON.stringify(step).includes("secrets."));
  assert.deepEqual(secretSteps.map((step) => step.name), ["Commit bun.nix or update auto-merge"]);
  for (const name of workflow.on.workflow_run.workflows) {
    const file = { CI: "ci", Parity: "parity", CodSpeed: "codspeed" }[name];
    assert.ok(file, `Unknown workflow ${name}`);
    const source = parse(readFileSync(new URL(`../.github/workflows/${file}.yml`, import.meta.url), "utf8"));
    assert.equal(source.name, name);
    assert.ok(source.on.pull_request, `${name} does not run on pull requests`);
  }
});

import { writeFileSync } from "node:fs";

// Dependabot pull requests merge once every check on the head commit has passed, including
// advisory checks such as Parity that main's ruleset does not require. GitHub auto-merge waits
// only for required checks, so it is armed only after every other check has finished, and
// disarmed whenever a check is pending again or has failed.
//
// Writes use the maintainer token: a bun.nix commit pushed with GITHUB_TOKEN starts no pull
// request checks, and a merge made with it starts no push workflows on main.

// Jobs behind `needs`, such as Parity's shards and its result job, have no check run until they
// queue, and a run that fails to start or waits for approval creates none, so green check runs
// alone do not prove a workflow passed. The latest run of each followed workflow on the head must
// have concluded successfully.
export const followedWorkflows = ["CI", "Parity", "CodSpeed"];
const passed = new Set(["SUCCESS", "NEUTRAL", "SKIPPED"]);
const pendingStatus = new Set(["PENDING", "EXPECTED"]);

const checksQuery = `query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      commits(last: 1) { nodes { commit { oid statusCheckRollup { contexts(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          __typename
          ... on CheckRun { name status conclusion isRequired(pullRequestNumber: $number) }
          ... on StatusContext { context state isRequired(pullRequestNumber: $number) }
        }
      } } } } }
    }
  }
}`;

function checkState(node) {
  if (node.__typename === "StatusContext") {
    if (node.state === "SUCCESS") return "pass";
    return pendingStatus.has(node.state) ? "pending" : "fail";
  }
  if (node.status !== "COMPLETED") return "pending";
  return passed.has(node.conclusion) ? "pass" : "fail";
}

export function mergeReadiness(checks, runs) {
  if (checks.length === 0) return { action: "wait", reason: "no checks have reported yet" };
  const failed = checks.filter((check) => check.state === "fail").map((check) => check.name);
  if (failed.length) return { action: "block", reason: `failed: ${failed.join(", ")}` };
  const unsuccessful = runs.filter((run) => run.status === "completed" && run.conclusion !== "success");
  if (unsuccessful.length) return { action: "block", reason: `concluded: ${unsuccessful.map((run) => `${run.name} ${run.conclusion}`).join(", ")}` };
  const missing = followedWorkflows.filter((name) => !runs.some((run) => run.name === name));
  if (missing.length) return { action: "wait", reason: `not started: ${missing.join(", ")}` };
  const running = runs.filter((run) => run.status !== "completed").map((run) => run.name);
  if (running.length) return { action: "wait", reason: `running: ${running.join(", ")}` };
  const pending = checks.filter((check) => check.state === "pending" && !check.required).map((check) => check.name);
  if (pending.length) return { action: "wait", reason: `pending: ${pending.join(", ")}` };
  return { action: "arm", reason: "every advisory check passed; auto-merge waits for required checks" };
}

async function readRuns(github, repo, sha, branch) {
  const runs = await github.paginate(github.rest.actions.listWorkflowRunsForRepo, { ...repo, head_sha: sha, event: "pull_request", branch });
  // A newer run of the same workflow on the head (a reopen, or one that replaced a cancelled run)
  // supersedes older ones; a rerun keeps its run id. Another pull request from the same commit
  // has its own branch, so it never stands in for this one.
  const repository = `${repo.owner}/${repo.repo}`;
  const latest = new Map();
  for (const run of runs) {
    if (!followedWorkflows.includes(run.name) || run.head_branch !== branch || run.head_repository?.full_name !== repository) continue;
    if (run.id > (latest.get(run.name)?.id ?? -1)) latest.set(run.name, run);
  }
  return [...latest.values()].map((run) => ({ name: run.name, status: run.status, conclusion: run.conclusion }));
}

async function readChecks(github, repo, pr) {
  const checks = [];
  let cursor = null;
  for (;;) {
    const result = await github.graphql(checksQuery, { ...repo, number: pr.number, cursor });
    const commit = result.repository.pullRequest.commits.nodes[0]?.commit;
    if (commit?.oid !== pr.sha) throw new Error(`Pull request #${pr.number} moved from ${pr.sha} while its checks were read.`);
    const contexts = commit.statusCheckRollup?.contexts;
    if (!contexts) return checks;
    for (const node of contexts.nodes) {
      checks.push({ name: node.name ?? node.context, state: checkState(node), required: node.isRequired === true });
    }
    if (!contexts.pageInfo.hasNextPage) return checks;
    cursor = contexts.pageInfo.endCursor;
  }
}

// Settlement rereads the pull request and checks it again, because a pull request can be
// retargeted or closed after it was found while keeping its head commit.
function ineligibility(pull, repository, sha) {
  if (pull.state !== "open") return `#${pull.number} is ${pull.state}`;
  if (pull.head.sha !== sha) return `#${pull.number} moved from ${sha} to ${pull.head.sha}`;
  if (pull.user.login !== "dependabot[bot]") return `#${pull.number} was not opened by Dependabot`;
  if (pull.head.repo?.full_name !== repository.full_name) return `#${pull.number} comes from another repository`;
  if (pull.base.ref !== repository.default_branch) return `#${pull.number} does not target ${repository.default_branch}`;
  return null;
}

// Returns the open Dependabot pull request whose head is the commit the triggering run checked,
// or null when the event is stale or belongs to anything else.
export async function findPullRequest({ github, context, core }) {
  const run = context.payload.workflow_run;
  const repository = context.payload.repository;
  const skip = (reason) => {
    core.info(`Skipping: ${reason}`);
    core.setOutput("number", "");
    return null;
  };
  if (run.event !== "pull_request" || run.head_repository?.full_name !== repository.full_name) return skip("not a same-repository pull request run");
  const pulls = await github.paginate(github.rest.pulls.list, {
    ...context.repo, state: "open", head: `${context.repo.owner}:${run.head_branch}`,
  });
  const pr = pulls.find((pull) => pull.head.sha === run.head_sha);
  if (!pr) return skip(`no open pull request has head ${run.head_sha}`);
  const reason = ineligibility(pr, repository, run.head_sha);
  if (reason) return skip(reason);
  const files = await github.paginate(github.rest.pulls.listFiles, { ...context.repo, pull_number: pr.number });
  const bunLock = files.some((file) => file.filename === "bun.lock" && file.status !== "removed");
  core.setOutput("number", String(pr.number));
  core.setOutput("sha", pr.head.sha);
  core.setOutput("bun-lock", String(bunLock));
  return { number: pr.number, sha: pr.head.sha, bunLock };
}

async function readFile(github, repo, path, ref) {
  const { data } = await github.rest.repos.getContent({ ...repo, path, ref, mediaType: { format: "raw" } });
  return String(data);
}

async function readOptionalFile(github, repo, path, ref) {
  try {
    return await readFile(github, repo, path, ref);
  } catch (error) {
    if (error.status === 404) return null;
    throw error;
  }
}

async function disarm(writer, core, current, reason) {
  await writer.graphql("mutation($id: ID!) { disablePullRequestAutoMerge(input: { pullRequestId: $id }) { clientMutationId } }", { id: current.node_id });
  core.notice(`Disarmed auto-merge on #${current.number}: ${reason}.`);
}

// Writes the pull request's bun.lock into the default-branch checkout, so bun2nix runs with the
// default branch's pinned flake and never executes pull request code.
export async function fetchBunLock({ github, context, sha, path }) {
  writeFileSync(path, await readFile(github, context.repo, "bun.lock", sha));
}

async function commitBunNix({ writer, context, core, pr, branch, contents }) {
  const repository = `${context.repo.owner}/${context.repo.repo}`;
  const result = await writer.graphql(`mutation($input: CreateCommitOnBranchInput!) {
    createCommitOnBranch(input: $input) { commit { oid signature { isValid } } }
  }`, { input: {
    branch: { repositoryNameWithOwner: repository, branchName: branch },
    expectedHeadOid: pr.sha,
    message: { headline: "chore(nix): regenerate bun.nix from bun.lock" },
    fileChanges: { additions: [{ path: "bun.nix", contents: Buffer.from(contents).toString("base64") }] },
  } });
  const commit = result.createCommitOnBranch.commit;
  if (commit.signature?.isValid !== true) throw new Error(`bun.nix commit ${commit.oid} on #${pr.number} lacks a verified signature.`);
  core.notice(`Committed the regenerated bun.nix to #${pr.number} as ${commit.oid}; its checks run again.`);
}

export async function settlePullRequest({ github, writer, context, core, pr, regeneratedBunNix }) {
  if (!writer) throw new Error("NIX_LOCK_REFRESH_TOKEN is unavailable to this run. Add it as a Dependabot secret as well as an Actions secret.");
  const repo = context.repo;
  const { data: current } = await github.rest.pulls.get({ ...repo, pull_number: pr.number });
  const reason = ineligibility(current, context.payload.repository, pr.sha);
  if (reason) {
    core.info(`Skipping: ${reason}.`);
    return "stale";
  }
  const armed = Boolean(current.auto_merge);
  if (regeneratedBunNix !== undefined && regeneratedBunNix !== await readOptionalFile(github, repo, "bun.nix", pr.sha)) {
    if (armed) await disarm(writer, core, current, "bun.nix is stale");
    await commitBunNix({ writer, context, core, pr, branch: current.head.ref, contents: regeneratedBunNix });
    return "committed";
  }
  const readiness = mergeReadiness(await readChecks(github, repo, pr), await readRuns(github, repo, pr.sha, current.head.ref));
  core.info(`#${pr.number} at ${pr.sha}: ${readiness.action} (${readiness.reason})`);
  if (readiness.action !== "arm") {
    if (armed) {
      await disarm(writer, core, current, readiness.reason);
      return "disarmed";
    }
    return readiness.action;
  }
  if (armed) return "armed";
  const merge = async () => {
    await writer.graphql(`mutation($id: ID!, $sha: GitObjectID!) {
      mergePullRequest(input: { pullRequestId: $id, expectedHeadOid: $sha, mergeMethod: SQUASH }) { clientMutationId }
    }`, { id: current.node_id, sha: pr.sha });
    core.notice(`Merged #${pr.number} at ${pr.sha}.`);
    return "merged";
  };
  // GitHub refuses to arm auto-merge on a pull request that can already merge, so a clean one
  // merges now, as `gh pr merge --auto` does. The refusal still covers a state that changed since.
  if (current.mergeable_state === "clean") return merge();
  try {
    await writer.graphql(`mutation($id: ID!, $sha: GitObjectID!) {
      enablePullRequestAutoMerge(input: { pullRequestId: $id, expectedHeadOid: $sha, mergeMethod: SQUASH }) { clientMutationId }
    }`, { id: current.node_id, sha: pr.sha });
    core.notice(`Armed auto-merge on #${pr.number} at ${pr.sha}.`);
    return "armed";
  } catch (error) {
    if (!/clean status/i.test(error.message)) throw error;
    return merge();
  }
}

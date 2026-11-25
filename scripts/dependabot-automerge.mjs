import { writeFileSync } from "node:fs";

// Dependabot pull requests merge once every check on the head commit has passed, including
// advisory checks such as Parity that main's ruleset does not require. GitHub auto-merge waits
// only for required checks, so it is armed only after every other check has finished, and
// disarmed whenever a check is pending again or has failed.
//
// Writes use the maintainer token: a bun.nix commit pushed with GITHUB_TOKEN starts no pull
// request checks, and a merge made with it starts no push workflows on main.

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

export function mergeReadiness(checks) {
  if (checks.length === 0) return { action: "wait", reason: "no checks have reported yet" };
  const failed = checks.filter((check) => check.state === "fail").map((check) => check.name);
  if (failed.length) return { action: "block", reason: `failed: ${failed.join(", ")}` };
  const pending = checks.filter((check) => check.state === "pending" && !check.required).map((check) => check.name);
  if (pending.length) return { action: "wait", reason: `pending: ${pending.join(", ")}` };
  return { action: "arm", reason: "every advisory check passed; auto-merge waits for required checks" };
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
  if (pr.user.login !== "dependabot[bot]") return skip(`#${pr.number} was not opened by Dependabot`);
  if (pr.base.ref !== repository.default_branch) return skip(`#${pr.number} does not target ${repository.default_branch}`);
  const files = await github.paginate(github.rest.pulls.listFiles, { ...context.repo, pull_number: pr.number });
  const bunLock = files.some((file) => file.filename === "bun.lock");
  core.setOutput("number", String(pr.number));
  core.setOutput("sha", pr.head.sha);
  core.setOutput("bun-lock", String(bunLock));
  return { number: pr.number, sha: pr.head.sha, bunLock };
}

async function readFile(github, repo, path, ref) {
  const { data } = await github.rest.repos.getContent({ ...repo, path, ref, mediaType: { format: "raw" } });
  return String(data);
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
  if (current.state !== "open" || current.head.sha !== pr.sha) {
    core.info(`#${pr.number} moved or closed; a newer run settles it.`);
    return "stale";
  }
  if (regeneratedBunNix !== undefined && regeneratedBunNix !== await readFile(github, repo, "bun.nix", pr.sha)) {
    await commitBunNix({ writer, context, core, pr, branch: current.head.ref, contents: regeneratedBunNix });
    return "committed";
  }
  const readiness = mergeReadiness(await readChecks(github, repo, pr));
  const armed = Boolean(current.auto_merge);
  core.info(`#${pr.number} at ${pr.sha}: ${readiness.action} (${readiness.reason})`);
  if (readiness.action !== "arm") {
    if (armed) {
      await writer.graphql("mutation($id: ID!) { disablePullRequestAutoMerge(input: { pullRequestId: $id }) { clientMutationId } }", { id: current.node_id });
      core.notice(`Disarmed auto-merge on #${pr.number}: ${readiness.reason}.`);
      return "disarmed";
    }
    return readiness.action;
  }
  if (armed) return "armed";
  try {
    await writer.graphql(`mutation($id: ID!, $sha: GitObjectID!) {
      enablePullRequestAutoMerge(input: { pullRequestId: $id, expectedHeadOid: $sha, mergeMethod: SQUASH }) { clientMutationId }
    }`, { id: current.node_id, sha: pr.sha });
    core.notice(`Armed auto-merge on #${pr.number} at ${pr.sha}.`);
    return "armed";
  } catch (error) {
    // GitHub refuses to arm auto-merge on a pull request that can already merge.
    if (!/clean status/i.test(error.message)) throw error;
    await writer.graphql(`mutation($id: ID!, $sha: GitObjectID!) {
      mergePullRequest(input: { pullRequestId: $id, expectedHeadOid: $sha, mergeMethod: SQUASH }) { clientMutationId }
    }`, { id: current.node_id, sha: pr.sha });
    core.notice(`Merged #${pr.number} at ${pr.sha}.`);
    return "merged";
  }
}

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const repository = process.env.GH_REPO;
const branch = process.env.BRANCH_NAME;
const title = process.env.RELEASE_TITLE;
if (!repository || !branch?.startsWith("release/app-v") || !title) {
  throw new Error("Release commit requires GH_REPO, BRANCH_NAME, and RELEASE_TITLE.");
}

const paths = run("git", ["diff", "--cached", "--name-only", "-z"]).split("\0").filter(Boolean);
if (paths.length === 0) throw new Error("No release files are staged.");
const additions = paths.map((path) => ({ path, contents: readFileSync(path).toString("base64") }));
const refs = JSON.parse(run("gh", ["api", `repos/${repository}/git/matching-refs/heads/${branch}`]));
let parent = refs.find((ref) => ref.ref === `refs/heads/${branch}`)?.object.sha;
const base = run("git", ["rev-parse", "HEAD"]).trim();
if (parent) {
  const pages = JSON.parse(run("gh", ["api", "--paginate", "--slurp", `repos/${repository}/compare/${base}...${parent}?page=1`]));
  for (const page of pages) {
    for (const commit of page.commits) {
      if (commit.commit.verification?.verified !== true) {
        throw new Error(`Release branch ${branch} contains unverified commit ${commit.sha}. Verify its signature or use a new signed branch before rerunning.`);
      }
    }
  }
} else {
  parent = base;
  run("gh", ["api", "--method", "POST", `repos/${repository}/git/refs`, "-f", `ref=refs/heads/${branch}`, "-f", `sha=${parent}`]);
}

const response = JSON.parse(run("gh", ["api", "graphql", "--input", "-"], JSON.stringify({
  query: "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid signature { isValid } } } }",
  variables: {
    input: {
      branch: { repositoryNameWithOwner: repository, refName: branch },
      expectedHeadOid: parent,
      message: { headline: title },
      fileChanges: { additions },
    },
  },
})));
const commit = response.data?.createCommitOnBranch?.commit;
if (commit?.signature?.isValid !== true) {
  throw new Error(`Release commit ${commit?.oid ?? "unknown"} on ${branch} lacks a verified signature. The branch may have advanced. Verify its signature or use a new signed branch before rerunning.`);
}
console.log(`Created verified release commit ${commit.oid} on ${branch}.`);

function run(command, args, input) {
  const result = spawnSync(command, args, { encoding: "utf8", input });
  if (result.status !== 0) throw new Error(`${command} failed: ${result.stderr || result.error?.message}`);
  return result.stdout;
}

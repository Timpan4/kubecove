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
if (!parent) {
  parent = run("git", ["rev-parse", "HEAD"]).trim();
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
if (commit?.signature?.isValid !== true) throw new Error("Release commit must have a verified signature.");
console.log(`Created verified release commit ${commit.oid} on ${branch}.`);

function run(command, args, input) {
  const result = spawnSync(command, args, { encoding: "utf8", input });
  if (result.status !== 0) throw new Error(`${command} failed: ${result.stderr || result.error?.message}`);
  return result.stdout;
}

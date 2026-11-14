const validationPaths = new Set([
  ".github/workflows/ci.yml", ".github/workflows/parity.yml",
  ".github/workflows/codspeed.yml", ".github/workflows/e2e.yml",
  ".github/workflows/tsrs.yml", ".github/workflows/server-packaging.yml",
]);
const activeStates = ["queued", "in_progress", "pending", "waiting", "requested"];

export async function cancelObsoleteRuns({ github, context, core }) {
  const repository = context.payload.repository;
  const repo = context.repo;
  async function closedPullRequest(run) {
    const headRepo = run.head_repository?.full_name;
    if (!headRepo || !run.head_branch) return false;
    const pulls = run.pull_requests?.length
      ? await Promise.all(run.pull_requests.map(async ({ number }) => (await github.rest.pulls.get({ ...repo, pull_number: number })).data))
      : await github.paginate(github.rest.pulls.list, { ...repo, state: "all", head: `${headRepo.split("/")[0]}:${run.head_branch}` });
    if (run.pull_requests?.length) return pulls.every((pr) => pr.state === "closed");
    const matching = pulls.filter((pr) => pr.head?.repo?.full_name === headRepo && pr.head.ref === run.head_branch);
    // A reopened or reused branch with an open PR still needs its validation.
    if (matching.some((pr) => pr.state !== "closed")) return false;
    for (const pr of matching) {
      if (pr.head.sha === run.head_sha) return true;
      const commits = await github.paginate(github.rest.pulls.listCommits, { ...repo, pull_number: pr.number });
      if (commits.some((commit) => commit.sha === run.head_sha)) return true;
    }
    return false;
  }
  // Finish pagination before cancelling. Removing runs while paging can skip rows.
  const pages = await Promise.all(activeStates.map((status) => github.paginate(
    github.rest.actions.listWorkflowRunsForRepo, { ...repo, status },
  )));
  const runs = new Map(pages.flat().map((run) => [run.id, run]));
  for (const candidate of runs.values()) {
    if (!validationPaths.has(candidate.path)) continue;
    try {
      const { data: run } = await github.rest.actions.getWorkflowRun({ ...repo, run_id: candidate.id });
      if (!activeStates.includes(run.status) || !validationPaths.has(run.path)) continue;
      let obsolete = false;
      if (run.event === "pull_request") {
        obsolete = await closedPullRequest(run);
      } else if (["push", "workflow_dispatch", "schedule"].includes(run.event) && run.head_repository?.full_name === repository.full_name) {
        if (run.head_branch === repository.default_branch) {
          const { data: branch } = await github.rest.repos.getBranch({ ...repo, branch: repository.default_branch });
          obsolete = Boolean(branch.commit?.sha) && branch.commit.sha !== run.head_sha;
        } else if (run.event === "workflow_dispatch") {
          let branchExists = false;
          try {
            await github.rest.repos.getBranch({ ...repo, branch: run.head_branch });
            branchExists = true;
          } catch (error) {
            if (error.status !== 404) throw error;
            // A dispatch can target a tag. Missing branches alone do not prove deletion.
            try {
              await github.rest.git.getRef({ ...repo, ref: `tags/${run.head_branch}` });
            } catch (tagError) {
              if (tagError.status !== 404) throw tagError;
              obsolete = true;
            }
          }
          if (branchExists) obsolete = await closedPullRequest(run);
        }
      }
      if (!obsolete) continue;
      await github.rest.actions.cancelWorkflowRun({ ...repo, run_id: run.id });
      core.info(`Cancelled obsolete validation run ${run.id}.`);
    } catch (error) {
      // A completed run can reject cancellation. Uncertain lookups retain checks.
      core.warning(`Could not reconcile validation run ${candidate.id}: ${error.message}`);
    }
  }
}

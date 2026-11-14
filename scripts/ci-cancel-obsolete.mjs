const validationPaths = new Set([
  ".github/workflows/ci.yml", ".github/workflows/parity.yml",
  ".github/workflows/codspeed.yml", ".github/workflows/e2e.yml",
  ".github/workflows/tsrs.yml", ".github/workflows/server-packaging.yml",
]);
const activeStates = ["queued", "in_progress", "pending", "waiting", "requested"];

export async function cancelObsoleteRuns({ github, context, core }) {
  const repository = context.payload.repository;
  const repo = context.repo;
  const reads = new Map();
  function readOnce(key, load) {
    if (!reads.has(key)) reads.set(key, Promise.resolve().then(load));
    return reads.get(key);
  }
  const branch = (name) => readOnce(`branch:${name}`, async () => (await github.rest.repos.getBranch({ ...repo, branch: name })).data);
  const jobs = (run) => readOnce(`jobs:${run.id}`, () => github.paginate(github.rest.actions.listJobsForWorkflowRun, { ...repo, run_id: run.id, filter: "latest" }));
  const mode = (run) => run.path === ".github/workflows/e2e.yml" && run.event !== "push" ? "matrix" : "primary";
  async function replacedOnMain(run, sha) {
    const replacements = await readOnce(`main-runs:${sha}`, () => github.paginate(
      github.rest.actions.listWorkflowRunsForRepo, { ...repo, branch: repository.default_branch, head_sha: sha },
    ));
    const visibleJobs = await jobs(run);
    // Nix depends on the CI matrix. Its absence leaves downstream selection unknown.
    if (run.path === ".github/workflows/ci.yml" && !visibleJobs.some((job) => job.name === "Nix build")) return false;
    const originalJobs = visibleJobs.filter((job) => job.conclusion !== "skipped");
    if (!originalJobs.length) return false;
    if (run.path === ".github/workflows/ci.yml" &&
        (originalJobs.find((job) => job.name === "Detect CI changes")?.conclusion !== "success" ||
         !originalJobs.some((job) => job.name.startsWith("CI ") || job.name === "Nix build"))) return false;
    if (run.path === ".github/workflows/codspeed.yml" &&
        (originalJobs.find((job) => job.name === "Detect benchmark changes")?.conclusion !== "success" ||
         !originalJobs.some((job) => job.name.startsWith("Run frontend benchmarks (") || job.name.startsWith("Run Rust benchmarks (")))) return false;
    for (const replacement of replacements) {
      if (replacement.id === run.id || replacement.path !== run.path || replacement.head_sha !== sha ||
          replacement.head_branch !== repository.default_branch || replacement.head_repository?.full_name !== repository.full_name ||
          (!activeStates.includes(replacement.status) && replacement.conclusion !== "success") ||
          !["push", "schedule", "workflow_dispatch"].includes(replacement.event) || mode(replacement) !== mode(run)) continue;
      const coverage = new Set((await jobs(replacement))
        .filter((job) => job.conclusion === "success" || activeStates.includes(job.status))
        .map((job) => job.name));
      if (originalJobs.every((job) => coverage.has(job.name))) return true;
    }
    return false;
  }
  async function closedPullRequest(run) {
    const headRepo = run.head_repository?.full_name;
    if (!headRepo || !run.head_branch) return false;
    const pulls = run.pull_requests?.length
      ? await Promise.all(run.pull_requests.map(({ number }) => readOnce(`pr:${number}`, async () => (await github.rest.pulls.get({ ...repo, pull_number: number })).data)))
      : await readOnce(`prs:${headRepo}:${run.head_branch}`, () => github.paginate(github.rest.pulls.list, { ...repo, state: "all", head: `${headRepo.split("/")[0]}:${run.head_branch}` }));
    if (run.pull_requests?.length) return pulls.every((pr) => pr.state === "closed");
    const matching = pulls.filter((pr) => pr.head?.repo?.full_name === headRepo && pr.head.ref === run.head_branch);
    // A reopened or reused branch with an open PR still needs its validation.
    if (matching.some((pr) => pr.state !== "closed")) return false;
    for (const pr of matching) {
      if (pr.head.sha === run.head_sha) return true;
      const commits = await readOnce(`commits:${pr.number}`, () => github.paginate(github.rest.pulls.listCommits, { ...repo, pull_number: pr.number }));
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
      if (run.event === "workflow_dispatch") {
        // Actions exposes a head name, not whether the dispatch selected a branch or tag.
        try {
          await readOnce(`tag:${run.head_branch}`, () => github.rest.git.getRef({ ...repo, ref: `tags/${run.head_branch}` }));
          continue;
        } catch (error) { if (error.status !== 404) throw error; }
      }
      let obsolete = false;
      if (run.event === "pull_request") {
        obsolete = await closedPullRequest(run);
      } else if (["push", "workflow_dispatch", "schedule"].includes(run.event) && run.head_repository?.full_name === repository.full_name) {
        if (run.head_branch === repository.default_branch) {
          const current = await branch(repository.default_branch);
          obsolete = Boolean(current.commit?.sha) && current.commit.sha !== run.head_sha && await replacedOnMain(run, current.commit.sha);
        } else if (run.event === "workflow_dispatch") {
          let branchExists = false;
          try {
            await branch(run.head_branch);
            branchExists = true;
          } catch (error) {
            if (error.status !== 404) throw error;
            obsolete = true;
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

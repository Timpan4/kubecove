// A pass record is separate from mbx's compiler cache. A cache miss always runs.
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";

const taskSteps = {
  rust: ["test-rust", "lint-rust"],
  "gpui-clippy": ["lint-gpui"],
  "gpui-test": ["test-gpui"],
  "server-backend": ["dependency-tree", "test-backend"],
  server: ["api-only", "browser-build", "test-server", "lint-server", "build-server", "http-lifecycle", "headless-report"],
};
const auditSteps = ["audit-coverage", "install-audit", "audit-desktop", "audit-gpui", "audit-server"];
const path = ".ci-rust-pass/record.json";
const { PASS_TASK: task, PASS_KEY: key, PASS_EVENT: event, PASS_CACHE_MATCH: matched } = process.env;
const environment = Object.fromEntries(["RUNNER_OS", "RUNNER_ARCH", "ImageOS", "ImageVersion"].map((name) => [name, process.env[name]]));
const hasIdentity = Object.values(environment).every(Boolean);
const prefix = hasIdentity ? `passed-${key}-${createHash("sha256").update(JSON.stringify(environment)).digest("hex")}-` : "";
const runPrefix = `${process.env.GITHUB_SERVER_URL}/${process.env.GITHUB_REPOSITORY}/actions/runs/`;
function validRun(value) {
  try {
    const url = new URL(value);
    return url.href === value && url.href.startsWith(runPrefix) && /^\d+\/attempts\/\d+$/.test(url.href.slice(runPrefix.length));
  } catch {
    return false;
  }
}
// A cache entry cannot be replaced, and a record can outlive a source job that
// failed in a post hook. Each run saves under its own key, so the newest entry
// under the prefix is the one a lookup restores, and a later pass supersedes it.
const keyFor = (run) => (prefix && validRun(run) ? `${prefix}${run.slice(runPrefix.length).replace("/attempts/", "-")}` : "");
const currentRun = `${runPrefix}${process.env.GITHUB_RUN_ID}/attempts/${process.env.GITHUB_RUN_ATTEMPT}`;

function readPass() {
  if (event !== "pull_request" || !matched) return undefined;
  try {
    const record = JSON.parse(readFileSync(path, "utf8"));
    if (record?.task !== task || record.key !== key || keyFor(record.run) !== matched ||
        !Object.entries(environment).every(([name, value]) => record.environment?.[name] === value)) return undefined;
    return record;
  } catch {
    // An unreadable or malformed cache record is no proof of a pass.
    return undefined;
  }
}

async function sourceJobPassed(record) {
  try {
    // Action post hooks run after the save step. A record alone cannot prove that
    // its source job finished successfully, including compiler-cache cleanup.
    if (!process.env.GITHUB_TOKEN) return false;
    const [run, , attempt] = record.run.slice(runPrefix.length).split("/");
    const url = `${process.env.GITHUB_API_URL}/repos/${process.env.GITHUB_REPOSITORY}/actions/runs/${run}/attempts/${attempt}/jobs`;
    const response = await fetch(url, { headers: {
      Accept: "application/vnd.github+json", Authorization: `Bearer ${process.env.GITHUB_TOKEN}`,
    } });
    if (!response.ok) return false;
    const { jobs, total_count: total } = await response.json();
    // Only a complete first page proves one job has this name. A run attempt
    // with more jobs than the API's default page runs the task again.
    if (total !== jobs.length) return false;
    const matches = jobs.filter((job) => job.name === `CI ${task}`);
    return matches.length === 1 && matches[0].status === "completed" && matches[0].conclusion === "success";
  } catch {
    return false;
  }
}

const [mode] = process.argv.slice(2);
if (!Object.hasOwn(taskSteps, task) || !new RegExp(`^mbx-ci-${task}-pass-[a-f0-9]{64}-[a-f0-9]{64}$`).test(key ?? "")) {
  // Invalid lookup inputs cannot justify a skip. Verification rejects them below.
  if (mode !== "decide") throw new Error("Invalid Rust task or cache key");
  appendFileSync(process.env.GITHUB_OUTPUT, "skip=false\n");
} else if (mode === "cache-key") {
  // Missing runner or run identity can never seed a shared, image-independent pass.
  const cacheKey = keyFor(currentRun);
  appendFileSync(process.env.GITHUB_OUTPUT, `key=${cacheKey}\nprefix=${cacheKey && prefix}\n`);
} else if (mode === "decide") {
  const record = readPass();
  const skip = Boolean(record && await sourceJobPassed(record));
  appendFileSync(process.env.GITHUB_OUTPUT, `skip=${skip}\n`);
  console.log(skip
    ? `${task}: ${matched} records ${record.run}; skipping build and test work.`
    : `${task}: no reusable pass under ${prefix || key}; running verification.`);
} else if (mode === "verify") {
  const steps = JSON.parse(process.env.PASS_STEPS);
  const decision = process.env.PASS_SKIP;
  const skip = decision === "true";
  if (!["true", "false"].includes(decision) || steps["decide-pass"]?.outputs?.skip !== decision || (skip && !readPass())) {
    throw new Error("Rust task reuse has no matching pass decision");
  }
  const requireOutcome = (id, outcome) => {
    if (steps[id]?.outcome !== outcome) throw new Error(`${task}: ${id} did not ${outcome}`);
  };
  for (const id of ["setup-rust", "rust-cache", "rust-pass", "pass-cache", "decide-pass"]) requireOutcome(id, "success");
  for (const id of ["setup-mbx", "activate-cargo", ...taskSteps[task]]) requireOutcome(id, skip ? "skipped" : "success");
  if (task === "rust") for (const id of auditSteps) requireOutcome(id, "success");
  for (const [id, step] of Object.entries(steps)) {
    if (["failure", "cancelled"].includes(step.conclusion)) throw new Error(`${task}: ${id} ${step.conclusion}`);
  }
  if (!skip) {
    if (!validRun(currentRun)) throw new Error("Missing workflow run identity");
    mkdirSync(".ci-rust-pass", { recursive: true });
    writeFileSync(path, `${JSON.stringify({ task, key, environment, run: currentRun })}\n`);
    console.log(`${task}: verified ${keyFor(currentRun) || key} in ${currentRun}`);
  }
} else {
  throw new Error("Expected cache-key, decide or verify");
}

import { appendFileSync, readFileSync } from "node:fs";
import { selectBenchmarks, frontendMatrix, selectionSummary } from "./codspeed-selection.mjs";

// Git and dependency-install failures are handled by the workflow shell too, so
// even failure to load this module runs both lanes without a name filter.
let result;
let matrix;
try {
  const fullReason = process.env.EVENT_NAME !== "pull_request" ? "full baseline run" :
    process.env.SELECTION_FALLBACK || undefined;
  const paths = fullReason ? [] : readFileSync(process.argv[2], "utf8").split("\0").filter(Boolean);
  result = await selectBenchmarks(paths, { fullReason });
  matrix = frontendMatrix(result.selected);
  if (process.env.RELEASE_PR === "true") {
    result = { selected: [], decisions: result.decisions.map(({ file }) => ({ file, run: false, reason: "labeled same-repository release PR" })), rust: false, rustReason: "labeled same-repository release PR" };
    matrix = { include: [] };
  }
} catch (error) {
  result = await selectBenchmarks([], { fullReason: `selection failed: ${error.message}` });
  matrix = { include: [{ group: 0, selection: "" }] };
}
appendFileSync(process.env.GITHUB_OUTPUT,
  `frontend=${result.selected.length > 0}\nrust=${result.rust}\nfrontend_matrix=${JSON.stringify(matrix)}\n`);
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, selectionSummary(result, matrix));

import { readFileSync } from "node:fs";
import { benchmarkSelection } from "./codspeed-benchmark-suites.mjs";

// Validate the producer's output with the benchmark consumer before publishing it.
const lines = readFileSync(process.argv[2], "utf8").trimEnd().split("\n");
if (lines.length !== 3) throw new Error("Expected three benchmark outputs");
const outputs = new Map(lines.map((line) => {
  const separator = line.indexOf("=");
  return [line.slice(0, separator), line.slice(separator + 1)];
}));
for (const key of ["frontend", "rust"]) {
  if (!["true", "false"].includes(outputs.get(key))) throw new Error(`Invalid ${key} flag`);
}
const matrix = JSON.parse(outputs.get("frontend_matrix"));
if (!matrix || !Array.isArray(matrix.include) ||
  (matrix.include.length > 0) !== (outputs.get("frontend") === "true")) {
  throw new Error("Benchmark matrix contradicts frontend flag");
}
for (const entry of matrix.include) {
  if (!entry || !Number.isSafeInteger(entry.group) || entry.group < 0 ||
    // oxlint-disable-next-line anti-slop/no-runtime-typeof -- Validate the serialized CI output at its boundary.
    typeof entry.selection !== "string") throw new Error("Invalid benchmark job");
  const selection = benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: entry.selection });
  if (selection.testNamePattern !== undefined) RegExp(selection.testNamePattern);
}

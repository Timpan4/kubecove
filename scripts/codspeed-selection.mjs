import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { benchmarkFiles, benchmarkGroups } from "./codspeed-benchmark-suites.mjs";

const repository = fileURLToPath(new URL("../", import.meta.url));
const sharedWorkflow = (path) => path === ".github/workflows/codspeed.yml" || path.startsWith(".github/actions/codspeed-run/");
const sharedSelector = (path) => ["scripts/codspeed-detect.mjs", "scripts/codspeed-selection.mjs", "scripts/codspeed-selection.test.mjs", "scripts/codspeed-workflow-regression.test.mjs"].includes(path);
const fixture = (path) => /(^|\/)fixtures\//.test(path);
const frontendGlobal = (path) => sharedWorkflow(path) ||
  /(^|\/)(package\.json|bun\.lock|(?:vitest|vite|svelte)\.config\.[^/]+|tsconfig[^/]*\.json|\.env(?:\.[^/]+)?)$/.test(path) ||
  path === "benchmarks/benchmark.ts" || path.startsWith("scripts/codspeed-");

export function rustAffected(path) {
  if (sharedWorkflow(path) || sharedSelector(path) || fixture(path)) return true;
  if (/^src-tauri\/(gpui|server)\//.test(path) || /^(e2e|tests)\//.test(path)) return false;
  return path.startsWith("src-tauri/") ||
    ["scripts/ci-rust-cache-key.mjs", "scripts/ci-mbx-cargo.mjs"].includes(path) ||
    /\.rs$|(^|\/)(Cargo\.(lock|toml)|rust-toolchain(\.toml)?)$|(^|\/)\.cargo\//.test(path);
}

// Vite's default resolve.extensions, the ones an import may leave out.
const resolvedExtensions = ["mjs", "js", "mts", "ts", "jsx", "tsx", "json"];

// Closures come from the current tree, so they cannot name a deleted file. When another file
// answers the same extensionless import, such as value/index.ts after value.ts is deleted, the
// importers stay unchanged and resolve, and nothing else reports the new implementation.
function replacedByAnotherFile(root, path) {
  const stem = new RegExp(`^(.*)\\.(${resolvedExtensions.join("|")})$`).exec(path)?.[1];
  if (stem === undefined || existsSync(join(root, path))) return false;
  const stems = [stem, `${stem}/index`, ...(stem.endsWith("/index") ? [stem.slice(0, -"/index".length)] : [])];
  return stems.some((candidate) => resolvedExtensions.some((extension) => existsSync(join(root, `${candidate}.${extension}`))));
}

export async function selectBenchmarks(paths, { root = repository, files = benchmarkFiles(), fullReason } = {}) {
  const global = paths.filter(frontendGlobal);
  const replaced = paths.filter((path) => replacedByAnotherFile(root, path));
  const reason = fullReason || (global.length ? `shared benchmark input: ${global.join(", ")}` : undefined) ||
    (replaced.length ? `deleted module that another file now resolves for: ${replaced.join(", ")}` : undefined);
  const rustPaths = paths.filter(rustAffected);
  const rust = Boolean(fullReason || rustPaths.length);
  const decisions = [];
  const closures = reason || !files.length ? new Map() : await (await import("./codspeed-import-closure.mjs")).importClosures(root, files);
  for (const file of files) {
    const closure = closures.get(file);
    const changed = closure ? paths.filter((path) => closure.paths.has(path)) : [];
    const why = reason || (closure.reasons.length ? `always run: ${closure.reasons.join("; ")}` : changed.length ? `import closure changed: ${changed.join(", ")}` : "no changed input in import closure");
    decisions.push({ file, run: Boolean(reason || closure.reasons.length || changed.length), reason: why });
  }
  return { selected: decisions.filter((entry) => entry.run).map((entry) => entry.file), decisions, rust, rustReason: fullReason || (rust ? `Rust lane input: ${rustPaths.join(", ")}` : "no Rust lane input changed") };
}

// Eight is the existing full-run matrix. Use its measured longest group as the
// target, then choose the smallest matrix that meets it for the selected files.
export function frontendMatrix(files) {
  if (!files.length) return { include: [] };
  try {
    const ceiling = Math.max(...benchmarkGroups(benchmarkFiles(), 8).map((group) => group.seconds));
    for (let count = 1; count <= 8; count++) {
      const groups = benchmarkGroups(files, count);
      if (count === 8 || groups.every((group) => group.seconds <= ceiling)) {
        return { include: groups.map((group, index) => ({ group: index, selection: JSON.stringify({ include: group.files, exclude: [], testNamePattern: group.testNamePattern }) })) };
      }
    }
  } catch {
    // No projection exists for a new benchmark. Run the entire selection in one
    // job without name filters until measured durations are added.
    return { include: [{ group: 0, selection: JSON.stringify({ include: files, exclude: [] }) }] };
  }
}

export function selectionSummary(result, matrix) {
  return ["## Benchmark selection", "", `Frontend jobs: ${matrix.include.length}`, "", "| Benchmark file | Decision | Reason |", "| --- | --- | --- |",
    ...result.decisions.map(({ file, run, reason }) => `| ${file} | ${run ? "Run" : "Skip"} | ${reason.replaceAll("|", "\\|").replaceAll("\n", " ")} |`), "",
    `Rust shards ${result.rust ? "run" : "skipped"}: ac, dl, es, hr, jn, other. ${result.rustReason}.`, ""].join("\n");
}

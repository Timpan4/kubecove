import { existsSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { isBuiltin } from "node:module";
import { isAbsolute, relative, resolve } from "node:path";
import { parseSync, resolveConfig, Visitor } from "vite";

// The benchmark configuration's own run-time reads that were checked by hand. This module
// only lists benchmarks/, and each benchmark file selects itself. A source change
// invalidates the audit and makes every benchmark run until reviewed again.
const auditedConfigReads = new Map([["scripts/codspeed-benchmark-suites.mjs", "637b42167de9beadc642e9f6fabc46f2ba0bad0f515222ba89534e2e68cff7bc"]]);

// Parse source without executing it or building benchmarks. Vite owns extension,
// directory, package and alias resolution, matching the benchmark configuration.
// Returns each file's closure and the extensions an import may leave out.
export async function importClosures(root, files) {
  const configFile = existsSync(resolve(root, "vitest.config.ts")) ? resolve(root, "vitest.config.ts") : false;
  // `vitest bench` loads its configuration in mode "test" unless --mode is passed, and
  // the workflow passes none. Vite's own default would be "development".
  const config = await resolveConfig({ root, configFile, envFile: false, logLevel: "silent", mode: "test" }, "serve");
  // This Vite API creates its own SSR environment without a server or watcher.
  const resolveId = config.createResolver();
  const modules = new Map();
  const local = (path) => relative(root, path).replaceAll("\\", "/");

  async function imports(file) {
    if (modules.has(file)) return modules.get(file);
    const result = { dependencies: [], reasons: [] };
    modules.set(file, result);
    try {
      if (file.endsWith(".json")) return result;
      if (!/\.[cm]?[jt]sx?$/.test(file)) throw new Error("unsupported source format");
      const parsed = parseSync(file, readFileSync(file, "utf8"));
      if (parsed.errors.length) throw new Error("source parse failed");
      const specifiers = new Set();
      const from = (node) => { if (node.source) specifiers.add(node.source.value); };
      const runtimeCode = (node) => {
        if (node.callee.type === "Identifier" && ["eval", "Function", "Worker", "SharedWorker"].includes(node.callee.name)) result.reasons.push(`runtime code (${node.callee.name})`);
      };
      new Visitor({
        ImportDeclaration: from,
        ExportNamedDeclaration: from,
        ExportAllDeclaration: from,
        ImportExpression(node) {
          if (node.source.type === "Literal") specifiers.add(node.source.value);
          else result.reasons.push("computed dynamic import");
        },
        // Globs, URL assets and runtime reads are outside an ESM closure.
        MetaProperty(node) { if (node.meta.name === "import") result.reasons.push("import.meta dependency"); },
        Identifier(node) { if (node.name === "require" || node.name === "Bun") result.reasons.push(`runtime dependency (${node.name})`); },
        CallExpression: runtimeCode,
        NewExpression: runtimeCode,
      }).visit(parsed.program);
      for (const specifier of specifiers) {
        if (isBuiltin(specifier)) {
          if (!/^(node:)?(assert(\/strict)?|util|path|buffer|url)$/.test(specifier)) result.reasons.push(`runtime builtin (${specifier})`);
          continue;
        }
        const resolved = await resolveId(specifier, file, false, true);
        if (!resolved || !isAbsolute(resolved) || resolved.includes("?")) {
          result.reasons.push(`unresolved import (${specifier})`);
          continue;
        }
        const path = local(resolved);
        if (path.includes("node_modules/")) continue;
        if (path.startsWith("../") || isAbsolute(path)) result.reasons.push(`import outside repository (${specifier})`);
        else result.dependencies.push(resolved);
      }
    } catch (error) {
      result.reasons.push(error.message);
    }
    return result;
  }

  // Vitest runs these files for every benchmark, and the configuration only names them.
  const setup = [];
  const setupReasons = [];
  for (const entry of [config.test?.setupFiles, config.test?.globalSetup].flat().filter(Boolean)) {
    const resolved = await resolveId(String(entry), resolve(root, "vitest.config.ts"), false, true);
    if (resolved && isAbsolute(resolved) && !resolved.includes("?")) setup.push(resolved);
    else setupReasons.push(`vitest.config.ts: unresolved setup file (${entry})`);
  }
  // The configuration runs before any benchmark and decides resolution. Its source and
  // imports are shared inputs, but a file it reads at run time is named nowhere, so a
  // read that was not checked by hand makes every benchmark run.
  const configModules = new Set([...(configFile ? [configFile] : []), ...config.configFileDependencies]);
  for (const path of configModules) {
    const source = local(path);
    if (source.includes("node_modules/")) continue;
    const audit = auditedConfigReads.get(source);
    if (audit && createHash("sha256").update(readFileSync(path)).digest("hex") === audit) continue;
    for (const reason of (await imports(path)).reasons) {
      // Configuration locates files through import.meta.url, which reads nothing.
      if (reason !== "import.meta dependency") setupReasons.push(`${source}: ${reason}`);
    }
  }

  const closures = new Map();
  for (const file of files) {
    const visited = new Set(configModules);
    const reasons = new Set(setupReasons);
    const pending = [resolve(root, file), ...setup];
    while (pending.length) {
      const path = pending.pop();
      if (visited.has(path)) continue;
      visited.add(path);
      const entry = await imports(path);
      for (const reason of entry.reasons) reasons.add(`${local(path)}: ${reason}`);
      pending.push(...entry.dependencies);
    }
    closures.set(file, { paths: new Set([...visited].map(local)), reasons: [...reasons] });
  }
  return { closures, extensions: config.resolve.extensions };
}

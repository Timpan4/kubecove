import { existsSync, readFileSync } from "node:fs";
import { isBuiltin } from "node:module";
import { isAbsolute, relative, resolve } from "node:path";
import { parseSync, resolveConfig, Visitor } from "vite";

// Parse source without executing it or building benchmarks. Vite owns extension,
// directory, package and alias resolution, matching the benchmark configuration.
export async function importClosures(root, files) {
  const config = await resolveConfig({
    root, configFile: existsSync(resolve(root, "vitest.config.ts")) ? resolve(root, "vitest.config.ts") : false,
    envFile: false, logLevel: "silent",
  }, "serve");
  // This Vite API creates its own SSR environment without a server or watcher.
  const resolveId = config.createResolver();
  const modules = new Map();

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
          if (!/^(node:)?(assert(\/strict)?|util|path|buffer)$/.test(specifier)) result.reasons.push(`runtime builtin (${specifier})`);
          continue;
        }
        const resolved = await resolveId(specifier, file, false, true);
        if (!resolved || !isAbsolute(resolved) || resolved.includes("?")) {
          result.reasons.push(`unresolved import (${specifier})`);
          continue;
        }
        const path = relative(root, resolved).replaceAll("\\", "/");
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

  const closures = new Map();
  for (const file of files) {
    const visited = new Set(config.configFileDependencies);
    const reasons = new Set(setupReasons);
    const pending = [resolve(root, file), ...setup];
    while (pending.length) {
      const path = pending.pop();
      if (visited.has(path)) continue;
      visited.add(path);
      const entry = await imports(path);
      const source = relative(root, path).replaceAll("\\", "/");
      for (const reason of entry.reasons) reasons.add(`${source}: ${reason}`);
      pending.push(...entry.dependencies);
    }
    closures.set(file, {
      paths: new Set([...visited].map((path) => relative(root, path).replaceAll("\\", "/"))),
      reasons: [...reasons],
    });
  }
  return closures;
}

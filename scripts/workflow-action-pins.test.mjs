import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

const workflows = new URL("../.github/workflows/", import.meta.url);

test("external workflow actions are pinned to commit SHAs", () => {
  const mutableReferences = [];

  for (const name of readdirSync(workflows).filter((entry) => entry.endsWith(".yml") || entry.endsWith(".yaml"))) {
    const lines = readFileSync(new URL(name, workflows), "utf8").split(/\r?\n/);
    for (const [index, line] of lines.entries()) {
      const reference = /^\s*(?:-\s+)?uses:\s+([^\s#]+)/.exec(line)?.[1];
      if (!reference || reference.startsWith("./") || /@[a-f0-9]{40}$/.test(reference)) continue;
      mutableReferences.push(`${name}:${index + 1}: ${reference}`);
    }
  }

  assert.deepEqual(mutableReferences, []);
});

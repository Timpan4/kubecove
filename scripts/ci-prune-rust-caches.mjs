// Deletes the Rust build caches on main that a newer cache of the same name has replaced.
//
// A push to main that changes a build saves its cache under a new key
// (scripts/ci-rust-cache-key.mjs) and leaves the previous cache in place. The caches in
// use take about 8 GB of the repository's 10 GB quota, so the leftovers exceed it. GitHub
// then evicts the least recently used caches, and those can be caches that pull requests
// still restore. A cache of the same name saved later replaces an earlier one even when
// the compiler changed: no job on main asks for the earlier key again.
//
// Usage: node scripts/ci-prune-rust-caches.mjs [--delete]
// Without --delete it lists what it would delete. It calls `gh`, whose token needs
// `actions: write` to delete.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

// The name in a key that ci-rust-cache-key.mjs wrote: mbx-<name>-<compiler digest>-<input digest>.
export const cacheName = (key) => /^mbx-(.+)-[0-9a-f]{64}-[0-9a-f]{64}$/.exec(key)?.[1];

// Caches of other refs stay: a release branch or a pull request restores its own before main's.
export function supersededCaches(caches) {
  const newest = new Map();
  const superseded = [];
  for (const cache of caches) {
    const name = cache.ref === "refs/heads/main" ? cacheName(cache.key) : undefined;
    if (name === undefined) continue;
    const kept = newest.get(name);
    if (kept === undefined) {
      newest.set(name, cache);
    } else if (Date.parse(cache.created_at) > Date.parse(kept.created_at)) {
      superseded.push(kept);
      newest.set(name, cache);
    } else {
      superseded.push(cache);
    }
  }
  return superseded;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const gh = (...args) => execFileSync("gh", ["api", ...args], { encoding: "utf8", maxBuffer: Infinity });
  // 100 is the largest page this endpoint returns.
  const pages = JSON.parse(gh("--paginate", "--slurp", "repos/{owner}/{repo}/actions/caches?per_page=100"));
  const superseded = supersededCaches(pages.flatMap((page) => page.actions_caches));
  const remove = process.argv.includes("--delete");
  for (const cache of superseded) {
    console.log(`${remove ? "Deleting" : "Would delete"} ${cache.key} (${Math.round(cache.size_in_bytes / 2 ** 20)} MiB, saved ${cache.created_at})`);
    if (!remove) continue;
    try {
      gh("--method", "DELETE", `repos/{owner}/{repo}/actions/caches/${cache.id}`);
    } catch (error) {
      // GitHub can evict a cache between the listing and this call. The sweep goes on.
      if (!String(error.stderr).includes("(HTTP 404)")) throw error;
    }
  }
  console.log(`Superseded Rust build caches on main: ${superseded.length}`);
}

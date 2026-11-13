// Writes the cache key of one Rust build to GITHUB_OUTPUT.
//
// The key names the compiler and the Git blob of every tracked input. A build whose inputs
// did not change restores its exact cache, and the cache action then skips the export and
// upload, which cost up to 200 s per job when every run saved. A changed build restores the
// newest cache that shares the prefix and saves its own.
//
// The runner image is not part of the key. GitHub runs two image versions side by side
// while it rolls one out, so with the image in the key every cache was saved once per
// image: about 14 GB against the repository's 10 GB quota, and GitHub then evicted caches
// that main still used. The image does not change what the cache holds: a job on one image
// that restored the other image's cache got the same 595 hits and 15 misses as an exact
// hit (CI runs 43411879023 and 43414482769).
//
// Usage: node scripts/ci-rust-cache-key.mjs <name> <pathspec>...
// Pathspecs are Git pathspecs, so ":!src-tauri/gpui" leaves a tree out. Pass the calling
// workflow as one of them: its build settings decide what the compiler produces.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync } from "node:fs";

const [name, ...pathspecs] = process.argv.slice(2);
if (!name || !pathspecs.length) throw new Error("Expected a cache name and at least one pathspec");

const capture = (command, args) => execFileSync(command, args, { encoding: "utf8", maxBuffer: Infinity });
const digest = (...parts) => createHash("sha256").update(parts.join("\n")).digest("hex");

const toolchain = digest(capture("rustc", ["-Vv"]));
const scripts = ["scripts/ci-rust-cache-key.mjs", "scripts/ci-mbx-cargo.mjs"];
// Each pathspec that names files must match some. The scripts always match, so a check of
// the combined listing would pass a mistyped pathspec and key the cache without that input.
for (const pathspec of pathspecs) {
  if (!pathspec.startsWith(":!") && !capture("git", ["ls-files", "--", pathspec])) throw new Error(`No tracked file matches: ${pathspec}`);
}
const inputs = capture("git", ["ls-files", "--stage", "--", ...scripts, ...pathspecs]);

// The prefix keeps the compiler, so a toolchain update does not download a cache it cannot use.
const prefix = `mbx-${name}-${toolchain}-`;
const key = `${prefix}${digest(inputs)}`;
const output = `key=${key}\nprefix=${prefix}\n`;
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, output);
else process.stdout.write(output);

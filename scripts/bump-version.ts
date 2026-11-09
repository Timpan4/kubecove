import { spawnSync } from "node:child_process";
import {
	RELEASE_TAG_PREFIX,
	assertVersionGreaterThanLatestTag,
	latestReleaseTagVersionFromRefs,
	readWorkspaceReleaseVersions,
	updateWorkspaceReleaseVersions,
} from "./release-versions";

const targetVersion = Bun.argv.slice(2).find((arg) => arg !== "--");

if (!targetVersion) {
	fail("Usage: bun scripts/bump-version.ts <version>");
}

const latestReleaseTagVersion = readLatestRemoteReleaseTagVersion();

try {
	assertVersionGreaterThanLatestTag(
		targetVersion,
		latestReleaseTagVersion,
		"target version",
	);
} catch (error) {
	fail(error instanceof Error ? error.message : String(error));
}

const before = readWorkspaceReleaseVersions();
const after = updateWorkspaceReleaseVersions(targetVersion);

console.log(
	`Bumped KubeCove from ${before.packageVersion} to ${after.packageVersion}.`,
);
console.log(latestReleaseTagVersion
	? `Latest release tag: ${RELEASE_TAG_PREFIX}${latestReleaseTagVersion}.`
	: "No existing release tags found.");
console.log("Updated package.json, src-tauri/tauri.conf.json, and the Cargo.toml and Cargo.lock files of src-tauri, src-tauri/server, and src-tauri/gpui when present.");

function readLatestRemoteReleaseTagVersion(): string | null {
	const result = spawnSync(
		"git",
		["ls-remote", "--tags", "origin", `refs/tags/${RELEASE_TAG_PREFIX}*`],
		{ encoding: "utf8", stdio: "pipe" },
	);

	if (result.status !== 0) {
		fail(`Could not read release tags from origin:\n${result.stderr || result.stdout}`);
	}

	return latestReleaseTagVersionFromRefs(result.stdout ?? "");
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

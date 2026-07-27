import { appendFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import {
	RELEASE_TAG_PREFIX,
	type ReleaseBump,
	computeNextReleaseVersion,
	latestReleaseTagVersionFromRefs,
	updateWorkspaceReleaseVersions,
} from "./release-versions";
import { updateChangelog, updateReleaseDocsVersion } from "./release-notes";

const bump = Bun.argv[2] as ReleaseBump | undefined;
const validBumps = new Set<ReleaseBump>(["patch", "minor", "major"]);

if (!bump || !validBumps.has(bump)) {
	fail("Usage: bun scripts/prepare-release-pr.ts <patch|minor|major>");
}

run("git", ["fetch", "--quiet", "--tags", "origin"]);

const latestTagVersion = readLatestRemoteReleaseTagVersion();
const currentVersion = latestTagVersion ?? "0.0.0";
const nextVersion = computeNextReleaseVersion(currentVersion, bump);
const latestTag = latestTagVersion ? `${RELEASE_TAG_PREFIX}${latestTagVersion}` : null;
const subjects = readReleaseSubjects(latestTag);
const today = new Date().toISOString().slice(0, 10);

updateWorkspaceReleaseVersions(nextVersion);
updateReleaseDocsVersion(nextVersion);
updateChangelog(nextVersion, today, subjects);

const tagName = `${RELEASE_TAG_PREFIX}${nextVersion}`;
const branchName = `release/${tagName}`;
const title = `🔧 release KubeCove v${nextVersion}`;

writeOutput("version", nextVersion);
writeOutput("tag", tagName);
writeOutput("branch", branchName);
writeOutput("title", title);

console.error(`Prepared ${title} from ${latestTag ?? "no prior release tag"}.`);

function readLatestRemoteReleaseTagVersion(): string | null {
	const refs = run("git", [
		"ls-remote",
		"--tags",
		"origin",
		`refs/tags/${RELEASE_TAG_PREFIX}*`,
	]).stdout;
	return latestReleaseTagVersionFromRefs(refs);
}

function readReleaseSubjects(latestTag: string | null): string[] {
	const range = latestTag ? `${latestTag}..HEAD` : "HEAD";
	const result = run("git", ["log", "--no-merges", "--format=%s", range]);
	return result.stdout
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(Boolean);
}

function writeOutput(key: string, value: string): void {
	const outputPath = process.env.GITHUB_OUTPUT;
	if (outputPath) appendFileSync(outputPath, `${key}=${value}\n`);
	console.log(`${key}=${value}`);
}

function run(command: string, args: string[]): { stdout: string; stderr: string } {
	const result = spawnSync(command, args, { encoding: "utf8", stdio: "pipe" });
	if (result.status !== 0) {
		fail(`Command failed: ${[command, ...args].join(" ")}\n${result.stderr || result.stdout}`);
	}
	return { stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

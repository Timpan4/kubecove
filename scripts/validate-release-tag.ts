import { spawnSync } from "node:child_process";

import {
	assertMatchingReleaseVersions,
	readWorkspaceReleaseVersions,
} from "./release-versions";

const tagName = process.env.TAG_NAME ?? "";
const refType = process.env.REF_TYPE ?? "";
const TAG_BRANCHES: Record<string, string> = { "app-v0.15.2": "release/0.15.x" };

try {
	const versions = readWorkspaceReleaseVersions();

	console.log(`package.json version: ${versions.packageVersion}`);
	if (versions.tauriVersion) console.log(`tauri.conf.json version: ${versions.tauriVersion}`);
	console.log(`Cargo.toml version: ${versions.cargoVersion}`);
	console.log(`release ref: ${refType} ${tagName}`);

	assertMatchingReleaseVersions(versions);

	if (refType !== "tag") {
		fail(
			"Release workflow must run from an app-v* tag. For manual dispatch, choose the existing release tag ref.",
		);
	}

	const expectedTagName = `app-v${versions.packageVersion}`;
	if (tagName !== expectedTagName) {
		fail(
			`Release tag must match app-v<package version>. Expected ${expectedTagName}, found ${tagName}.`,
		);
	}

	assertTagTargetsReleaseBranch(tagName);
} catch (error) {
	fail(error instanceof Error ? error.message : String(error));
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

// Main no longer builds Tauri, so the 0.15.x pitstop ships from the head of its maintenance branch.
const RELEASE_BRANCH = TAG_BRANCHES[tagName] ?? "main";

function assertTagTargetsReleaseBranch(tagName: string): void {
	run("git", ["fetch", "--quiet", "--no-tags", "origin", `${RELEASE_BRANCH}:refs/remotes/origin/${RELEASE_BRANCH}`]);

	const tagCommit = run("git", ["rev-parse", `${tagName}^{commit}`]).trim();
	const branchCommit = run("git", ["rev-parse", `origin/${RELEASE_BRANCH}^{commit}`]).trim();

	console.log(`release tag commit: ${shortSha(tagCommit)}`);
	console.log(`origin/${RELEASE_BRANCH} commit: ${shortSha(branchCommit)}`);

	if (tagCommit === branchCommit) return;

	fail(
		[
			`Release tag ${tagName} must point to the current origin/${RELEASE_BRANCH} commit.`,
			`Found tag commit ${tagCommit}, but origin/${RELEASE_BRANCH} is ${branchCommit}.`,
			"Create releases through the reviewed release PR flow so branch protection remains the publication gate.",
		].join("\n"),
	);
}

function run(command: string, args: string[]): string {
	const result = spawnSync(command, args, {
		encoding: "utf8",
		stdio: ["ignore", "pipe", "pipe"],
	});

	if (result.status === 0) return result.stdout ?? "";

	const errorDetails = [
		`Command failed: ${command} ${args.join(" ")}`,
		result.stderr?.trim() || (result.error ? `Spawn error: ${result.error.message}` : "Unknown error"),
	];

	fail(errorDetails.filter(Boolean).join("\n"));
}

function shortSha(sha: string): string {
	return sha.slice(0, 12);
}

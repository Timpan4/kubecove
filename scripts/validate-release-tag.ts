import { spawnSync } from "node:child_process";

import {
	assertMatchingReleaseVersions,
	readWorkspaceReleaseVersions,
} from "./release-versions";

const tagName = process.env.TAG_NAME ?? "";
const refType = process.env.REF_TYPE ?? "";

try {
	const versions = readWorkspaceReleaseVersions();

	console.log(`package.json version: ${versions.packageVersion}`);
	console.log(`tauri.conf.json version: ${versions.tauriVersion}`);
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

	assertTagTargetsOriginMain(tagName);
} catch (error) {
	fail(error instanceof Error ? error.message : String(error));
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

function assertTagTargetsOriginMain(tagName: string): void {
	run("git", ["fetch", "--quiet", "--no-tags", "origin", "main:refs/remotes/origin/main"]);

	const tagCommit = run("git", ["rev-parse", `${tagName}^{commit}`]).trim();
	const originMainCommit = run("git", ["rev-parse", "origin/main^{commit}"]).trim();

	console.log(`release tag commit: ${shortSha(tagCommit)}`);
	console.log(`origin/main commit: ${shortSha(originMainCommit)}`);

	if (tagCommit === originMainCommit) return;

	fail(
		[
			`Release tag ${tagName} must point to the current origin/main commit.`,
			`Found tag commit ${tagCommit}, but origin/main is ${originMainCommit}.`,
			"Create releases through the reviewed release PR flow so branch protection remains the publication gate.",
		].join("\n"),
	);
}

function run(command: string, args: string[]): string {
	const result = spawnSync(command, args, {
		encoding: "utf8",
		stdio: ["ignore", "pipe", "pipe"],
	});

	if (result.status === 0) return result.stdout;

	fail(
		[
			`Command failed: ${command} ${args.join(" ")}`,
			result.stderr.trim(),
		]
			.filter(Boolean)
			.join("\n"),
	);
}

function shortSha(sha: string): string {
	return sha.slice(0, 12);
}

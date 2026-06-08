import { spawnSync } from "node:child_process";
import {
	RELEASE_TAG_PREFIX,
	assertMatchingReleaseVersions,
	assertVersionGreaterThanLatestTag,
	latestReleaseTagVersionFromRefs,
	readWorkspaceReleaseVersions,
} from "./release-versions";
import { readChangelogReleaseBody } from "./release-notes";

const releasePrTitle = process.env.RELEASE_PR_TITLE ?? "";
const releasePrLabels = (process.env.RELEASE_PR_LABELS ?? "").split(",");
const releaseSha = process.env.GITHUB_SHA ?? "HEAD";

if (!releasePrLabels.includes("release")) {
	fail("Refusing to tag: associated PR is missing the release label.");
}

const versions = readWorkspaceReleaseVersions();
assertMatchingReleaseVersions(versions);
readChangelogReleaseBody(versions.packageVersion);

const tagName = `${RELEASE_TAG_PREFIX}${versions.packageVersion}`;
const releaseName = `KubeCove v${versions.packageVersion}`;

if (!releasePrTitle.includes(releaseName)) {
	fail(`Refusing to tag: release PR title must include "${releaseName}".`);
}

const latestReleaseTagVersion = readLatestRemoteReleaseTagVersion();
assertVersionGreaterThanLatestTag(
	versions.packageVersion,
	latestReleaseTagVersion,
	"release version",
);
assertMissingRemoteTag(tagName);

run("git", ["tag", "-a", tagName, releaseSha, "-m", releaseName], { inherit: true });
run("git", ["push", "origin", tagName], { inherit: true });
console.log(`Pushed ${tagName} for ${releaseSha}.`);

function readLatestRemoteReleaseTagVersion(): string | null {
	const refs = run("git", [
		"ls-remote",
		"--tags",
		"origin",
		`refs/tags/${RELEASE_TAG_PREFIX}*`,
	]).stdout;
	return latestReleaseTagVersionFromRefs(refs);
}

function assertMissingRemoteTag(tagName: string): void {
	const result = run("git", ["ls-remote", "--exit-code", "--tags", "origin", `refs/tags/${tagName}`], {
		allowFailure: true,
	});
	if (result.status === 0) fail(`Remote tag already exists on origin: ${tagName}`);
	if (result.status !== 2) {
		fail(`Could not check remote tag ${tagName} on origin:\n${result.stderr || result.stdout}`);
	}
}

function run(
	command: string,
	args: string[],
	options: { allowFailure?: boolean; inherit?: boolean } = {},
): { stdout: string; stderr: string; status: number | null } {
	const result = spawnSync(command, args, {
		encoding: "utf8",
		stdio: options.inherit ? "inherit" : "pipe",
	});
	const commandResult = {
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
		status: result.status,
	};
	if (!options.allowFailure && result.status !== 0) {
		fail(`Command failed: ${[command, ...args].join(" ")}\n${commandResult.stderr || commandResult.stdout}`);
	}
	return commandResult;
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

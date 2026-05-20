import { spawnSync } from "node:child_process";
import {
	RELEASE_TAG_PREFIX,
	assertMatchingReleaseVersions,
	assertVersionGreaterThanLatestTag,
	latestReleaseTagVersionFromRefs,
	parseCargoPackageVersion,
	requireVersion,
} from "./release-versions";

type Mode = "check" | "dry-run" | "release";

type CommandResult = {
	stdout: string;
	stderr: string;
	status: number | null;
};

const mode = (Bun.argv[2] ?? "check") as Mode;
const validModes = new Set<Mode>(["check", "dry-run", "release"]);

if (!validModes.has(mode)) {
	fail(`Unknown release mode: ${mode}\nUsage: bun scripts/release-beta.ts [check|dry-run|release]`);
}

fetchReleaseRefs();

const originMainSha = run("git", ["rev-parse", "origin/main"]).stdout.trim();
const latestReleaseTagVersion = readLatestRemoteReleaseTagVersion();
const packageJson = JSON.parse(readRemoteFile("package.json")) as { version?: string };
const tauriConfig = JSON.parse(readRemoteFile("src-tauri/tauri.conf.json")) as { version?: string };
const cargoToml = readRemoteFile("src-tauri/Cargo.toml");

const { packageVersion, tauriVersion, cargoVersion } = parseRemoteVersions();

try {
	assertMatchingReleaseVersions({ packageVersion, tauriVersion, cargoVersion });
} catch {
	fail([
		"Release versions on origin/main must match:",
		`  origin/main:package.json: ${packageVersion}`,
		`  origin/main:src-tauri/tauri.conf.json: ${tauriVersion}`,
		`  origin/main:src-tauri/Cargo.toml: ${cargoVersion}`,
	].join("\n"));
}

try {
	assertVersionGreaterThanLatestTag(
		packageVersion,
		latestReleaseTagVersion,
		"origin/main release version",
	);
} catch (error) {
	fail(error instanceof Error ? error.message : String(error));
}

const tagName = `${RELEASE_TAG_PREFIX}${packageVersion}`;
const releaseName = `KubeCove v${packageVersion}`;

assertMissingLocalTag(tagName);
assertMissingRemoteTag(tagName);

if (mode === "check") {
	console.log(`Release check passed for ${releaseName} (${tagName}) at origin/main ${shortSha(originMainSha)}.`);
	console.log(latestReleaseTagVersion
		? `Latest release tag: ${RELEASE_TAG_PREFIX}${latestReleaseTagVersion}.`
		: "No existing release tags found.");
	process.exit(0);
}

if (mode === "dry-run") {
	console.log(`Release dry run passed for ${releaseName}.`);
	console.log(latestReleaseTagVersion
		? `Latest release tag: ${RELEASE_TAG_PREFIX}${latestReleaseTagVersion}.`
		: "No existing release tags found.");
	console.log(`Would create annotated tag: ${tagName} -> origin/main ${shortSha(originMainSha)}`);
	console.log(`Would push tag: git push origin ${tagName}`);
	console.log("GitHub Actions would run tests, build installers, and publish a GitHub Release.");
	process.exit(0);
}

run("git", ["tag", "-a", tagName, originMainSha, "-m", releaseName], { inherit: true });
run("git", ["push", "origin", tagName], { inherit: true });
console.log(`Pushed ${tagName} for origin/main ${shortSha(originMainSha)}.`);
console.log("GitHub Actions will run tests, build installers, and publish a GitHub Release.");

function fetchReleaseRefs(): void {
	run("git", ["fetch", "--quiet", "--no-tags", "origin", "main:refs/remotes/origin/main"]);
}

function readLatestRemoteReleaseTagVersion(): string | null {
	const refs = run("git", [
		"ls-remote",
		"--tags",
		"origin",
		`refs/tags/${RELEASE_TAG_PREFIX}*`,
	]).stdout;
	return latestReleaseTagVersionFromRefs(refs);
}

function readRemoteFile(path: string): string {
	return run("git", ["show", `origin/main:${path}`]).stdout;
}

function parseRemoteVersions(): {
	packageVersion: string;
	tauriVersion: string;
	cargoVersion: string;
} {
	try {
		return {
			packageVersion: requireVersion("origin/main:package.json", packageJson.version),
			tauriVersion: requireVersion(
				"origin/main:src-tauri/tauri.conf.json",
				tauriConfig.version,
			),
			cargoVersion: parseCargoPackageVersion(
				cargoToml,
				"origin/main:src-tauri/Cargo.toml",
			),
		};
	} catch (error) {
		fail(error instanceof Error ? error.message : String(error));
	}
}

function assertMissingLocalTag(tagName: string): void {
	const result = run("git", ["rev-parse", "--verify", "--quiet", `refs/tags/${tagName}`], {
		allowFailure: true,
	});
	if (result.status === 0) fail(`Local tag already exists: ${tagName}`);
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

function shortSha(sha: string): string {
	return sha.slice(0, 12);
}

function run(
	command: string,
	args: string[],
	options: { allowFailure?: boolean; inherit?: boolean } = {},
): CommandResult {
	const result = spawnSync(command, args, {
		encoding: "utf8",
		stdio: options.inherit ? "inherit" : "pipe",
	});

	const commandText = [command, ...args].join(" ");
	const commandResult = {
		stdout: result.stdout ?? "",
		stderr: result.stderr ?? "",
		status: result.status,
	};

	if (!options.allowFailure && result.status !== 0) {
		fail(`Command failed: ${commandText}\n${commandResult.stderr || commandResult.stdout}`);
	}

	return commandResult;
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

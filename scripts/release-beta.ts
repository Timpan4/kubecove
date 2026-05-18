import { spawnSync } from "node:child_process";

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
const packageJson = JSON.parse(readRemoteFile("package.json")) as { version?: string };
const tauriConfig = JSON.parse(readRemoteFile("src-tauri/tauri.conf.json")) as { version?: string };
const cargoToml = readRemoteFile("src-tauri/Cargo.toml");

const packageVersion = requireVersion("origin/main:package.json", packageJson.version);
const tauriVersion = requireVersion("origin/main:src-tauri/tauri.conf.json", tauriConfig.version);
const cargoVersion = parseCargoVersion(cargoToml);

if (packageVersion !== tauriVersion || packageVersion !== cargoVersion) {
	fail([
		"Release versions on origin/main must match:",
		`  origin/main:package.json: ${packageVersion}`,
		`  origin/main:src-tauri/tauri.conf.json: ${tauriVersion}`,
		`  origin/main:src-tauri/Cargo.toml: ${cargoVersion}`,
	].join("\n"));
}

const tagName = `app-v${packageVersion}`;
const releaseName = `KubeCove v${packageVersion}`;

assertMissingLocalTag(tagName);
assertMissingRemoteTag(tagName);

if (mode === "check") {
	console.log(`Release check passed for ${releaseName} (${tagName}) at origin/main ${shortSha(originMainSha)}.`);
	process.exit(0);
}

if (mode === "dry-run") {
	console.log(`Release dry run passed for ${releaseName}.`);
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

function readRemoteFile(path: string): string {
	return run("git", ["show", `origin/main:${path}`]).stdout;
}

function requireVersion(source: string, version: string | undefined): string {
	if (!version) fail(`${source} is missing a version.`);
	if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
		fail(`${source} version must be semver-like. Found: ${version}`);
	}
	return version;
}

function parseCargoVersion(contents: string): string {
	const match = contents.match(/^version\s*=\s*"([^"]+)"/m);
	if (!match) fail("origin/main:src-tauri/Cargo.toml is missing package version.");
	return requireVersion("origin/main:src-tauri/Cargo.toml", match[1]);
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

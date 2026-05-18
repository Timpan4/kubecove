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

const packageJson = await Bun.file("package.json").json() as { version?: string };
const tauriConfig = await Bun.file("src-tauri/tauri.conf.json").json() as { version?: string };
const cargoToml = await Bun.file("src-tauri/Cargo.toml").text();

const packageVersion = requireVersion("package.json", packageJson.version);
const tauriVersion = requireVersion("src-tauri/tauri.conf.json", tauriConfig.version);
const cargoVersion = parseCargoVersion(cargoToml);

if (packageVersion !== tauriVersion || packageVersion !== cargoVersion) {
	fail([
		"Release versions must match:",
		`  package.json: ${packageVersion}`,
		`  src-tauri/tauri.conf.json: ${tauriVersion}`,
		`  src-tauri/Cargo.toml: ${cargoVersion}`,
	].join("\n"));
}

const tagName = `app-v${packageVersion}`;
const releaseName = `KubeCove v${packageVersion}`;

assertOnMain();
assertHeadMatchesOriginMain();
assertCleanWorktree();
assertMissingLocalTag(tagName);
assertMissingRemoteTag(tagName);
runProjectChecks();

if (mode === "check") {
	console.log(`Release check passed for ${releaseName} (${tagName}).`);
	process.exit(0);
}

if (mode === "dry-run") {
	console.log(`Release dry run passed for ${releaseName}.`);
	console.log(`Would create annotated tag: ${tagName}`);
	console.log(`Would push tag: git push origin ${tagName}`);
	process.exit(0);
}

run("git", ["tag", "-a", tagName, "-m", releaseName], { inherit: true });
run("git", ["push", "origin", tagName], { inherit: true });
console.log(`Pushed ${tagName}. GitHub Actions will create a draft beta release.`);

function requireVersion(source: string, version: string | undefined): string {
	if (!version) fail(`${source} is missing a version.`);
	if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
		fail(`${source} version must be semver-like. Found: ${version}`);
	}
	return version;
}

function parseCargoVersion(contents: string): string {
	const match = contents.match(/^version\s*=\s*"([^"]+)"/m);
	if (!match) fail("src-tauri/Cargo.toml is missing package version.");
	return requireVersion("src-tauri/Cargo.toml", match[1]);
}

function assertOnMain(): void {
	const branch = run("git", ["branch", "--show-current"]);
	if (branch.stdout.trim() !== "main") {
		fail(`Release must run from main. Current branch: ${branch.stdout.trim() || "(detached)"}`);
	}
}

function assertHeadMatchesOriginMain(): void {
	run("git", ["fetch", "--quiet", "origin", "main"]);
	const head = run("git", ["rev-parse", "HEAD"]).stdout.trim();
	const originMain = run("git", ["rev-parse", "origin/main"]).stdout.trim();

	if (head !== originMain) {
		fail("Release HEAD must match origin/main. Pull or push main before creating the release tag.");
	}
}

function assertCleanWorktree(): void {
	const status = run("git", ["status", "--short", "--untracked-files=all"]);
	if (status.stdout.trim()) {
		fail(`Release requires a clean worktree:\n${status.stdout}`);
	}
}

function assertMissingLocalTag(tagName: string): void {
	const result = run("git", ["rev-parse", "--verify", "--quiet", `refs/tags/${tagName}`], {
		allowFailure: true,
	});
	if (result.status === 0) fail(`Local tag already exists: ${tagName}`);
}

function assertMissingRemoteTag(tagName: string): void {
	const result = run("git", ["ls-remote", "--exit-code", "--tags", "origin", tagName], {
		allowFailure: true,
	});
	if (result.status === 0) fail(`Remote tag already exists on origin: ${tagName}`);
	if (result.status !== 2) {
		fail(`Could not check remote tag ${tagName} on origin:\n${result.stderr || result.stdout}`);
	}
}

function runProjectChecks(): void {
	console.log("Running release checks: bun run check");
	run("bun", ["run", "check"], { inherit: true });
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

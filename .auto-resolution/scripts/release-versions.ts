import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const RELEASE_TAG_PREFIX = "app-v";

export type ReleaseVersions = {
	packageVersion: string;
	tauriVersion: string;
	cargoVersion: string;
};

export type ReleaseBump = "patch" | "minor" | "major";

export function computeNextReleaseVersion(
	latestVersion: string,
	bump: ReleaseBump,
): string {
	const parsed = parseSemver(latestVersion);

	if (bump === "major") return `${parsed.major + 1}.0.0`;
	if (bump === "minor") return `${parsed.major}.${parsed.minor + 1}.0`;
	return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}`;
}

export function readWorkspaceReleaseVersions(root = "."): ReleaseVersions {
	const packageJson = JSON.parse(
		readFileSync(join(root, "package.json"), "utf8"),
	) as { version?: string };
	const tauriConfig = JSON.parse(
		readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
	) as { version?: string };
	const cargoToml = readFileSync(
		join(root, "src-tauri", "Cargo.toml"),
		"utf8",
	);

	return {
		packageVersion: requireVersion("package.json", packageJson.version),
		tauriVersion: requireVersion("src-tauri/tauri.conf.json", tauriConfig.version),
		cargoVersion: parseCargoPackageVersion(cargoToml, "src-tauri/Cargo.toml"),
	};
}

export function parseCargoPackageVersion(
	contents: string,
	source = "Cargo.toml",
): string {
	let inPackageSection = false;

	for (const line of contents.split(/\r?\n/)) {
		const trimmed = line.trim();
		const section = trimmed.match(/^\[([^\]]+)\]$/);

		if (section) {
			inPackageSection = section[1] === "package";
			continue;
		}

		if (!inPackageSection) continue;

		const version = trimmed.match(/^version\s*=\s*"([^"]+)"/);
		if (version) return requireVersion(source, version[1]);
	}

	throw new Error(`${source} is missing package version.`);
}

export function requireVersion(source: string, version: string | undefined): string {
	if (!version) throw new Error(`${source} is missing a version.`);
	if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
		throw new Error(`${source} version must be semver-like. Found: ${version}`);
	}
	return version;
}

export function assertMatchingReleaseVersions(versions: ReleaseVersions): void {
	const { packageVersion, tauriVersion, cargoVersion } = versions;

	if (packageVersion === tauriVersion && packageVersion === cargoVersion) return;

	throw new Error(
		[
			"Release versions must match:",
			`  package.json: ${packageVersion}`,
			`  src-tauri/tauri.conf.json: ${tauriVersion}`,
			`  src-tauri/Cargo.toml: ${cargoVersion}`,
		].join("\n"),
	);
}

export function compareSemverVersions(left: string, right: string): number {
	const a = parseSemver(left);
	const b = parseSemver(right);
	const coreCompare =
		compareNumber(a.major, b.major) ||
		compareNumber(a.minor, b.minor) ||
		compareNumber(a.patch, b.patch);
	if (coreCompare !== 0) return coreCompare;
	return comparePrerelease(a.prerelease, b.prerelease);
}

export function assertVersionGreaterThanLatestTag(
	version: string,
	latestTagVersion: string | null,
	source = "release version",
): void {
	requireVersion(source, version);
	if (!latestTagVersion) return;
	requireVersion("latest release tag", latestTagVersion);
	if (compareSemverVersions(version, latestTagVersion) > 0) return;

	throw new Error(
		`${source} ${version} must be greater than latest release tag ${RELEASE_TAG_PREFIX}${latestTagVersion}.`,
	);
}

export function releaseTagVersionFromRef(ref: string): string | null {
	const tagName = ref
		.trim()
		.replace(/\^\{\}$/, "")
		.split("/")
		.at(-1);
	if (!tagName?.startsWith(RELEASE_TAG_PREFIX)) return null;
	const version = tagName.slice(RELEASE_TAG_PREFIX.length);
	return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)
		? version
		: null;
}

export function latestReleaseTagVersionFromRefs(refs: string): string | null {
	const versions = refs
		.split(/\r?\n/)
		.map((line) => line.trim().split(/\s+/).at(-1) ?? "")
		.map(releaseTagVersionFromRef)
		.filter((version): version is string => Boolean(version));

	return versions.reduce<string | null>(
		(latest, version) =>
			latest === null || compareSemverVersions(version, latest) > 0
				? version
				: latest,
		null,
	);
}

export function updateWorkspaceReleaseVersions(
	version: string,
	root = ".",
): ReleaseVersions {
	const targetVersion = requireVersion("target version", version);
	const packagePath = join(root, "package.json");
	const tauriPath = join(root, "src-tauri", "tauri.conf.json");
	const cargoPath = join(root, "src-tauri", "Cargo.toml");
	const cargoLockPath = join(root, "src-tauri", "Cargo.lock");

	const packageJson = JSON.parse(readFileSync(packagePath, "utf8")) as {
		version?: string;
	};
	packageJson.version = targetVersion;
	writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);

	const tauriConfig = JSON.parse(readFileSync(tauriPath, "utf8")) as {
		version?: string;
	};
	tauriConfig.version = targetVersion;
	writeFileSync(tauriPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

	const cargoToml = readFileSync(cargoPath, "utf8");
	writeFileSync(
		cargoPath,
		replaceCargoPackageVersion(cargoToml, targetVersion, cargoPath),
	);

	if (existsSync(cargoLockPath)) {
		const cargoLock = readFileSync(cargoLockPath, "utf8");
		writeFileSync(
			cargoLockPath,
			replaceCargoLockPackageVersion(cargoLock, targetVersion),
		);
	}

	return {
		packageVersion: targetVersion,
		tauriVersion: targetVersion,
		cargoVersion: targetVersion,
	};
}

function replaceCargoPackageVersion(
	contents: string,
	version: string,
	source = "Cargo.toml",
): string {
	let inPackageSection = false;
	const lines = contents.split(/(\r?\n)/);

	for (let index = 0; index < lines.length; index += 2) {
		const line = lines[index] ?? "";
		const trimmed = line.trim();
		const section = trimmed.match(/^\[([^\]]+)\]$/);

		if (section) {
			inPackageSection = section[1] === "package";
			continue;
		}

		if (!inPackageSection) continue;

		if (/^\s*version\s*=\s*"[^"]+"/.test(line)) {
			lines[index] = line.replace(
				/^(\s*version\s*=\s*)"[^"]+"/,
				`$1"${version}"`,
			);
			return lines.join("");
		}
	}

	throw new Error(`${source} is missing package version.`);
}

function replaceCargoLockPackageVersion(
	contents: string,
	version: string,
): string {
	const packageHeader = 'name = "kubecove"';
	const lines = contents.split(/(\r?\n)/);

	for (let index = 0; index < lines.length; index += 2) {
		const line = lines[index] ?? "";
		if (line.trim() !== packageHeader) continue;

		for (let cursor = index + 2; cursor < lines.length; cursor += 2) {
			const nextLine = lines[cursor] ?? "";
			if (/^\[\[package\]\]/.test(nextLine.trim())) break;
			if (/^\s*version\s*=\s*"[^"]+"/.test(nextLine)) {
				lines[cursor] = nextLine.replace(
					/^(\s*version\s*=\s*)"[^"]+"/,
					`$1"${version}"`,
				);
				return lines.join("");
			}
		}
	}

	throw new Error(
		'src-tauri/Cargo.lock is missing [[package]] entry with name = "kubecove".',
	);
}

function compareNumber(left: number, right: number): number {
	return left === right ? 0 : left > right ? 1 : -1;
}

function parseSemver(version: string): {
	major: number;
	minor: number;
	patch: number;
	prerelease: string[];
} {
	const cleanVersion = requireVersion("semver", version).split("+")[0] ?? version;
	const hyphenIndex = cleanVersion.indexOf("-");
	const core =
		hyphenIndex === -1 ? cleanVersion : cleanVersion.slice(0, hyphenIndex);
	const prereleaseText =
		hyphenIndex === -1 ? undefined : cleanVersion.slice(hyphenIndex + 1);
	const [major, minor, patch] = (core ?? "").split(".").map(Number);
	return {
		major,
		minor,
		patch,
		prerelease: prereleaseText ? prereleaseText.split(".") : [],
	};
}

function comparePrerelease(left: string[], right: string[]): number {
	if (left.length === 0 && right.length === 0) return 0;
	if (left.length === 0) return 1;
	if (right.length === 0) return -1;

	const maxLength = Math.max(left.length, right.length);
	for (let index = 0; index < maxLength; index += 1) {
		const a = left[index];
		const b = right[index];
		if (a === undefined) return -1;
		if (b === undefined) return 1;
		if (a === b) continue;

		const aNumber = /^\d+$/.test(a) ? Number(a) : null;
		const bNumber = /^\d+$/.test(b) ? Number(b) : null;
		if (aNumber !== null && bNumber !== null) {
			return compareNumber(aNumber, bNumber);
		}
		if (aNumber !== null) return -1;
		if (bNumber !== null) return 1;
		return a.localeCompare(b) > 0 ? 1 : -1;
	}

	return 0;
}

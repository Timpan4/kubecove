import { readFileSync } from "node:fs";
import { join } from "node:path";

export type ReleaseVersions = {
	packageVersion: string;
	tauriVersion: string;
	cargoVersion: string;
};

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

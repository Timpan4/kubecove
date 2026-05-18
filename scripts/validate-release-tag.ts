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
} catch (error) {
	fail(error instanceof Error ? error.message : String(error));
}

function fail(message: string): never {
	console.error(message);
	process.exit(1);
}

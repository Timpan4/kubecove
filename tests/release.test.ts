import { describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
	assertVersionGreaterThanLatestTag,
	assertMatchingReleaseVersions,
	compareSemverVersions,
	latestReleaseTagVersionFromRefs,
	parseCargoPackageVersion,
	updateWorkspaceReleaseVersions,
} from "../scripts/release-versions";

describe("release version helpers", () => {
	test("reads the package version from Cargo.toml", () => {
		expect(
			parseCargoPackageVersion(`
[package]
name = "kubecove"
version = "0.1.0"
description = "A context-first desktop workspace for Kubernetes operations"

[dependencies]
example = "9.9.9"
`),
		).toBe("0.1.0");
	});

	test("ignores dependency versions before returning Cargo.toml package version", () => {
		expect(
			parseCargoPackageVersion(`
[workspace.dependencies]
example = "9.9.9"

[package]
name = "kubecove"
version = "0.2.0"
`),
		).toBe("0.2.0");
	});

	test("parses CRLF Cargo.toml package versions", () => {
		expect(
			parseCargoPackageVersion(
				'[package]\r\nname = "kubecove"\r\nversion = "0.1.0"\r\n',
			),
		).toBe("0.1.0");
	});

	test("fails when Cargo.toml package version is missing", () => {
		expect(() =>
			parseCargoPackageVersion(`
[package]
name = "kubecove"

[dependencies]
example = "9.9.9"
`),
		).toThrow("Cargo.toml is missing package version.");
	});

	test("fails when package and Cargo versions differ", () => {
		expect(() =>
			assertMatchingReleaseVersions({
				packageVersion: "0.1.0",
				tauriVersion: "0.1.0",
				cargoVersion: "0.2.0",
			}),
		).toThrow("Release versions must match");
	});

	test("orders semver versions without string sorting mistakes", () => {
		expect(compareSemverVersions("0.10.0", "0.2.0")).toBe(1);
		expect(compareSemverVersions("1.0.0", "1.0.0-rc.1")).toBe(1);
		expect(compareSemverVersions("1.0.0-beta.2", "1.0.0-beta.10")).toBe(-1);
	});

	test("finds the latest app release version from git tag refs", () => {
		const refs = [
			"abc123\trefs/tags/app-v0.1.0",
			"def456\trefs/tags/app-v0.10.0",
			"fed321\trefs/tags/app-v0.10.0^{}",
			"aaa111\trefs/tags/not-app-v9.9.9",
		].join("\n");

		expect(latestReleaseTagVersionFromRefs(refs)).toBe("0.10.0");
	});

	test("requires release versions to be newer than the latest app tag", () => {
		expect(() =>
			assertVersionGreaterThanLatestTag("0.2.0", "0.2.0", "target version"),
		).toThrow("must be greater than latest release tag");
		expect(() =>
			assertVersionGreaterThanLatestTag("0.2.1", "0.2.0", "target version"),
		).not.toThrow();
	});

	test("updates all local version files for a bump", () => {
		const root = mkdtempSync(join(tmpdir(), "kubecove-release-"));
		mkdirSync(join(root, "src-tauri"), { recursive: true });
		writeFileSync(
			join(root, "package.json"),
			'{\n  "name": "kubecove",\n  "version": "0.1.0"\n}\n',
		);
		writeFileSync(
			join(root, "src-tauri", "tauri.conf.json"),
			'{\n  "productName": "KubeCove",\n  "version": "0.1.0"\n}\n',
		);
		writeFileSync(
			join(root, "src-tauri", "Cargo.toml"),
			'[package]\nname = "kubecove"\nversion = "0.1.0"\n\n[dependencies]\nexample = "9.9.9"\n',
		);
		writeFileSync(
			join(root, "src-tauri", "Cargo.lock"),
			'[[package]]\nname = "kubecove"\nversion = "0.1.0"\n',
		);

		updateWorkspaceReleaseVersions("0.2.0", root);

		expect(JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version).toBe("0.2.0");
		expect(JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8")).version).toBe("0.2.0");
		expect(readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8")).toContain('version = "0.2.0"');
		expect(readFileSync(join(root, "src-tauri", "Cargo.lock"), "utf8")).toContain('version = "0.2.0"');
	});
});

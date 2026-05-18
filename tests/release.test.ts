import { describe, expect, test } from "bun:test";
import {
	assertMatchingReleaseVersions,
	parseCargoPackageVersion,
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
});

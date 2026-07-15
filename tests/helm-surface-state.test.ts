import { describe, expect, test } from "bun:test";
import type { HelmReleaseSummary } from "../src/lib/types";
import { helmStatusTone } from "../src/features/helm/helpers";
import {
	buildHelmReleaseState,
	resolveHelmNamespace,
	resolveTargetHelmRelease,
	selectedHelmReleaseExists,
	selectedHelmReleasePath,
} from "../src/features/helm/surfaceState";

function release(
	name: string,
	namespace: string,
	overrides: Partial<HelmReleaseSummary> = {},
): HelmReleaseSummary {
	return {
		cluster: "kind-dev",
		name,
		namespace,
		storageKind: "Secret",
		storageName: `sh.helm.release.v1.${name}.v1`,
		age: "1d",
		...overrides,
	};
}

describe("Helm surface state", () => {
	test("renders every pending release status with the warning tone", () => {
		expect(helmStatusTone("pending-install")).toBe("warning");
		expect(helmStatusTone("pending-upgrade")).toBe("warning");
		expect(helmStatusTone("pending-rollback")).toBe("warning");
	});

	test("filters release metadata and namespaces while keeping unfiltered rail counts", () => {
		const releases = [
			release("api", "prod", { chart: "payments", status: "deployed" }),
			release("worker", "ops", { appVersion: "2.4.0" }),
			release("web", "prod", { storageName: "custom-storage" }),
		];

		expect(buildHelmReleaseState(releases, "payments").filtered).toEqual([releases[0]]);
		expect(buildHelmReleaseState(releases, "2.4.0").filtered).toEqual([releases[1]]);
		expect(buildHelmReleaseState(releases, "custom-storage").filtered).toEqual([releases[2]]);
		expect(buildHelmReleaseState(releases, "", "prod").filtered).toEqual([
			releases[0],
			releases[2],
		]);
		expect(buildHelmReleaseState(releases, "worker", "prod").filtered).toEqual([]);
		expect(buildHelmReleaseState(releases, "payments", "prod").groups).toEqual([
			{ namespace: "ops", releases: [releases[1]] },
			{ namespace: "prod", releases: [releases[0], releases[2]] },
		]);
		expect(resolveHelmNamespace(releases, "prod")).toBe("prod");
		expect(resolveHelmNamespace(releases, "missing")).toBeNull();
		expect(resolveHelmNamespace(releases, null)).toBeNull();
	});

	test("resolves handoff targets and mirrors selection into path state", () => {
		const releases = [release("api", "prod"), release("api", "dev")];

		expect(resolveTargetHelmRelease(releases, { name: "api", namespace: "dev" })).toBe(releases[1]);
		expect(resolveTargetHelmRelease(releases, { name: "missing" })).toBeNull();
		expect(selectedHelmReleasePath(releases[1])).toEqual({ name: "api", namespace: "dev" });
		expect(selectedHelmReleasePath(null)).toBeNull();
	});

	test("clears selections that disappear after a refresh", () => {
		const selected = release("api", "prod");
		expect(selectedHelmReleaseExists([selected], selected)).toBe(true);
		expect(selectedHelmReleaseExists([release("worker", "prod")], selected)).toBe(false);
		expect(selectedHelmReleaseExists(undefined, selected)).toBe(true);
	});
});

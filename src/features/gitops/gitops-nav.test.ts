import type { FluxResourceKind } from "@/lib/types";
import {
	fluxKindFromLabel,
	isFluxKindLabel,
	normalizeArgoKindLabel,
} from "./gitops-nav";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: T): void;
	toEqual(expected: T): void;
};

function fluxKind(kind: string): FluxResourceKind {
	return {
		group: "source.toolkit.fluxcd.io",
		version: "v1",
		apiVersion: "source.toolkit.fluxcd.io/v1",
		kind,
		plural: `${kind.toLowerCase()}s`,
		namespaced: true,
		category: "source",
	};
}

describe("GitOps navigation labels", () => {
	test("normalizes new and legacy Argo CD labels", () => {
		expect(normalizeArgoKindLabel("Argo CD Applications")).toBe(
			"applications",
		);
		expect(normalizeArgoKindLabel("Applications")).toBe("applications");
		expect(normalizeArgoKindLabel("Argo CD ApplicationSets")).toBe(
			"applicationSets",
		);
		expect(normalizeArgoKindLabel("ApplicationSets")).toBe(
			"applicationSets",
		);
		expect(normalizeArgoKindLabel("Argo CD AppProjects")).toBe("appProjects");
		expect(normalizeArgoKindLabel("AppProjects")).toBe("appProjects");
	});

	test("maps polished and legacy Flux labels to installed kinds", () => {
		const kinds = [fluxKind("GitRepository"), fluxKind("HelmRelease")];

		expect(
			fluxKindFromLabel("Flux Git Repositories", kinds)?.kind,
		).toBe("GitRepository");
		expect(fluxKindFromLabel("Flux GitRepositories", kinds)?.kind).toBe(
			"GitRepository",
		);
		expect(fluxKindFromLabel("Flux Helm Releases", kinds)?.kind).toBe(
			"HelmRelease",
		);
		expect(isFluxKindLabel("Flux HelmReleases")).toBe(true);
	});
});

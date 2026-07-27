import { createDevMockTauriClient } from "./tauri-dev-mocks";
import type { DiscoveredResourceKind } from "./types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

describe("browser dev custom resources mock", () => {
	test("lists CRD-backed kinds only", async () => {
		const client = createDevMockTauriClient();
		const kinds = await client.invoke<DiscoveredResourceKind[]>("list_resource_kinds");

		expect(kinds.some((kind) => kind.kind === "Deployment")).toBe(false);
		expect(kinds.map((kind) => kind.kind)).toEqual([
			"Application",
			"ApplicationSet",
			"Kustomization",
			"HelmRelease",
		]);
	});

	test("filters present custom resources by namespace", async () => {
		const client = createDevMockTauriClient();

		const present = await client.invoke<DiscoveredResourceKind[]>(
			"list_present_custom_resource_kinds",
			{ namespaces: ["argocd"] },
		);
		const absent = await client.invoke<DiscoveredResourceKind[]>(
			"list_present_custom_resource_kinds",
			{ namespaces: ["no-crds-here"] },
		);

		expect(present.map((kind) => kind.kind)).toEqual(["Application"]);
		expect(absent).toEqual([]);
	});
});

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

describe("startup request wiring", () => {
	test("bootstrap and active source consumers use the same query contract", () => {
		const app = readFileSync("src/app/svelte/App.svelte", "utf8");
		expect(app).toContain("queryClient.fetchQuery(kubeconfigSourcesQueryOptions(liveSessionClient))");
		for (const path of [
			"src/app/svelte/WorkspaceShell.svelte",
			"src/features/workspaces/WorkspaceLauncher.svelte",
			"src/app/svelte/KubeconfigSettings.svelte",
		]) {
			const source = readFileSync(path, "utf8");
			expect(source).toContain("createQuery(() => kubeconfigSourcesQueryOptions(client))");
			expect(source).not.toContain("getKubeconfigSources");
		}
	});

	test("every provider consumer waits for namespace discovery to settle", () => {
		for (const path of [
			"src/app/svelte/SidebarTree.svelte",
			"src/app/svelte/CommandPalette.svelte",
			"src/features/workspaces/WorkspaceOverview.svelte",
			"src/features/gitops/GitOpsSurface.svelte",
		]) {
			const source = readFileSync(path, "utf8");
			for (const command of ["detectArgoCD", "detectFlux"]) {
				const enabled = source.match(new RegExp(`queryFn: \\(\\) => ${command}\\([^\\n]+\\n\\s*enabled: ([^,]+),`))?.[1];
				expect(enabled).toContain("sourceReady");
				expect(enabled).toContain("!namespacesQuery.isPending");
			}
		}
	});

	test("both custom-resource prewarm queries wait for source readiness", () => {
		const source = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		for (const query of ["workspaceCustomResourcePrewarmQuery", "presentCustomResourceKindsQuery"]) {
			const enabled = source.split(`const ${query} =`)[1]?.match(/enabled:([^,]+),/)?.[1];
			expect(enabled).toContain("workspaceReadContext.sourceReady");
			expect(enabled).toContain("showCustomResources");
		}
	});
});

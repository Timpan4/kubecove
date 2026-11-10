import { expect, test } from "bun:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createServer } from "vite";
import { readinessTimeoutMs } from "../../.agents/skills/verify-kubecove/scripts/processes";

test("temporary browser files cannot become stylesheet dependencies", async () => {
	const root = resolve(import.meta.dir, "../..");
	const directory = await mkdtemp(join(root, "e2e/parity/.source-"));
	const probe = ["[--parity", "outside-source:1]"].join("-");
	try {
		await writeFile(join(directory, "Preferences"), probe);
		const server = await createServer({
			root,
			cacheDir: join(directory, "vite"),
			server: { middlewareMode: true, watch: null, hmr: false },
			optimizeDeps: { noDiscovery: true, include: [] },
		});
		try {
			const stylesheet = await server.transformRequest("/src/App.css");
			expect(stylesheet?.code).toContain(".justify-between");
			expect(stylesheet?.code.includes("--parity-outside-source")).toBe(false);
			expect(server.environments.client.moduleGraph.getModulesByFile(join(directory, "Preferences"))?.size ?? 0).toBe(0);
		} finally {
			await server.close();
		}
	} finally {
		await rm(directory, { recursive: true });
	}
}, readinessTimeoutMs);

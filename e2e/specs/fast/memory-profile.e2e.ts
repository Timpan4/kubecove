import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { describe, it } from "mocha";

describe("browser heap observations", () => {
	it("keeps normal instrumentation absent and observes retained Query data through CDP", async () => {
		const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
		if (!artifacts) throw new Error("Artifact directory is required");
		await browser.url("/");
		await browser.execute(() => localStorage.clear());
		await browser.refresh();
		await $("#workspace-name").waitForDisplayed();
		expect(await browser.execute(() => Object.hasOwn(window, "__KUBECOVE_MEMORY_PROFILE__"))).toBe(false);
		await $("#workspace-name").setValue("Heap observation");
		const create = await $("button=Create workspace");
		await create.waitForEnabled();
		await create.click();
		await $("button=Resources").click();
		await $('button[aria-label^="Open resource "]').waitForDisplayed();
		const samples: object[] = [];
		async function sample(stage: string) {
			const group = "kubecove-memory-observation";
			try {
				const heap = await browser.sendCommandAndGetResult("Runtime.getHeapUsage", {});
				const prototype = await browser.sendCommandAndGetResult("Runtime.evaluate", {
					// Use the loaded module URL, including Vite's version query, to keep constructor identity.
					expression: "(async () => { const url = performance.getEntriesByType('resource').find(entry => entry.name.includes('/@tanstack_svelte-query.js?'))?.name; if (!url) throw new Error('Loaded query module is unavailable'); return (await import(url)).Query.prototype; })()",
					awaitPromise: true, objectGroup: group,
				});
				if (!prototype.result?.objectId) throw new Error("Query prototype is unavailable");
				const queries = await browser.sendCommandAndGetResult("Runtime.queryObjects", { prototypeObjectId: prototype.result.objectId, objectGroup: group });
				if (!queries.objects?.objectId) throw new Error("Heap Query objects are unavailable");
				const counts = await browser.sendCommandAndGetResult("Runtime.callFunctionOn", {
					objectId: queries.objects.objectId, returnByValue: true,
					// Only fixed roots and counts leave the page; no query key or payload is returned.
					functionDeclaration: `function () {
						const roots = new Map();
						for (const query of this) {
							const first = query.queryKey[0];
							const root = ["resources", "resource-metrics", "resource-topology"].includes(first) ? first : "other";
							const row = roots.get(root) || { root, objects: 0, arrayItems: 0, jsonBytes: 0 };
							row.objects++;
							if (Array.isArray(query.state.data)) row.arrayItems += query.state.data.length;
							if (query.state.data !== undefined) row.jsonBytes += new TextEncoder().encode(JSON.stringify(query.state.data)).byteLength;
							roots.set(root, row);
						}
						return [...roots.values()].sort((a, b) => a.root.localeCompare(b.root));
					}`,
				});
				if (counts.exceptionDetails || !Array.isArray(counts.result?.value) || counts.result.value.length === 0) throw new Error("Heap Query aggregation did not find the loaded application's queries");
				if (!counts.result.value.some((row: { root: string; arrayItems: number }) => row.root === "resources" && row.arrayItems > 0)) throw new Error("The populated resource Query data path is unavailable");
				samples.push({ stage, heap, queryStateData: counts.result.value });
			} finally {
				await browser.sendCommandAndGetResult("Runtime.releaseObjectGroup", { objectGroup: group });
			}
		}
		await sample("resources-open");
		await $('[aria-label="Open workspaces"]').click();
		await $("#workspace-name").waitForDisplayed();
		await sample("inactive-at-launcher");
		await writeFile(join(artifacts, "browser-query-heap.json"), `${JSON.stringify({ engine: "Chromium-browser-mocks", measurement: "Runtime.queryObjects(Query.prototype) and Query.state.data", limitations: ["Normal mixed browser mocks only", "CDP object inspection can trigger collection", "JSON bytes are not heap ownership sizes", "Counts include retained Query objects outside the query cache", "Native WebKit heap is unavailable separately"], samples }, null, 2)}\n`);
	});
});

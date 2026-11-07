import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import { checkLogCoverage } from "./web-log-coverage";

/** The ownerless StorageClass kind group on the Cluster Overview map. */
function ownerlessStorageGroup(page: Page) {
	return page.locator(".svelte-flow__node-standaloneKindGroup").filter({ has: page.getByTitle("Ownerless StorageClass", { exact: true }) });
}

export async function checkCoverage(page: Page, check: string) {
	switch (check) {
		case "event-summary": {
			const panel = page.getByRole("tabpanel", { name: "Events", exact: true });
			for (const [label, value] of [["Total", "2 events"], ["Warnings", "1"], ["Sources", "1"], ["Namespace", "payments"]]) {
				const text = await panel.getByText(label, { exact: true }).first().locator("..").textContent();
				if (!text?.replace(/\s+/g, "").endsWith(value.replace(/\s+/g, ""))) throw new Error(`Wrong ${label} event summary`);
			}
			break;
		}
		case "session-action-failure":
			await page.getByText("Live-session action failed", { exact: true }).waitFor();
			break;
		case "import-skip": {
			const apply = page.getByRole("button", { name: "Apply import", exact: true });
			if (!await apply.isDisabled()) throw new Error("All-Skip import enables Apply");
			const bounds = await apply.boundingBox();
			if (!bounds) throw new Error("Import preview is missing");
			await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
			await apply.waitFor();
			break;
		}
		case "rbac-user": case "rbac-group": case "rbac-sa": {
			const text = await page.locator("section").filter({ has: page.getByText("Latest verification", { exact: true }) }).last().textContent();
			const expected = check === "rbac-user" ? ['{"kind":"user","username":"alice","groups":["developers","system:authenticated"]}', '"apiGroup":"apps"', '"resource":"deployments"', '"namespace":"payments"', '"subresource":"scale"', '"name":"payments-api"'] : check === "rbac-group" ? ['{"kind":"group","group":"developers"}', '"kind":"nonResource"', '"nonResourceUrl":"/healthz"'] : ['{"kind":"serviceAccount","name":"payments-api","namespace":"payments"}', '"verb":"delete"', '"resource":"pods"'];
			if (expected.some((value) => !text?.includes(value))) throw new Error("Latest verification shows stale identity or target fields");
			if (check === "rbac-group" && text?.includes('"resource":')) throw new Error("URL target retained resource fields");
			break;
		}
		case "rbac-pages": {
			const labels = await page.getByRole("button", { name: /^Role\/coverage-role-/ }).allTextContents();
			if (labels.length !== 51 || new Set(labels).size !== labels.length) throw new Error("Load more skipped or duplicated RBAC objects");
			break;
		}
		case "helm-card-resources": case "helm-list-resources":
			await page.getByRole("button", { name: check === "helm-card-resources" ? "View resources" : "Resources", exact: true }).first().click();
			if (await page.getByRole("textbox", { name: "Search resources" }).inputValue() !== "metrics-gateway") throw new Error("Wrong release opened");
			break;
		case "emit-log": case "log-wrap": case "log-scroll-back": case "log-jump":
			await checkLogCoverage(page, check); break;
		case "action-filter":
			await page.locator('[data-resource-action="replicas"]').waitFor();
			if (await page.locator('[data-resource-action="rollout"]').count()) throw new Error("Action filter retained Rollout");
			break;
		case "replicas-reset":
			if (await page.getByLabel("Desired replicas", { exact: true }).inputValue() !== "2") throw new Error("Revert or Discard retained the replica draft");
			if (await page.getByRole("button", { name: "Check changes", exact: true }).count()) throw new Error("Discard retained pending actions");
			break;
		case "diff-compact":
			if ((await page.locator("div.font-mono.text-xs.leading-relaxed").textContent())?.includes("apiVersion: apps/v1")) throw new Error("Collapse retained the full diff");
			break;
		case "palette-outside": {
			const bounds = await page.getByRole("dialog").boundingBox();
			if (!bounds) throw new Error("Palette is missing");
			await page.mouse.click(bounds.x / 2, bounds.y / 2);
			break;
		}
		case "log-stacked":
		case "log-gutter": {
			const bounds = await page.locator("[data-log-row]").first().boundingBox();
			// LogEntry uses a 17px line, a bottom border, and 4px or 3px vertical padding.
			if (bounds?.height !== (check === "log-stacked" ? 26 : 24)) throw new Error("Log layout did not change its rendered row spacing");
			break;
		}
		case "log-one-line": {
			const rows = page.locator("[data-log-row]");
			await rows.first().waitFor();
			await rows.first().click();
			await rows.first().getByRole("button", { name: "Copy line", exact: true }).waitFor();
			await page.getByRole("button", { name: "Expand all", exact: true }).click();
			if (await page.getByRole("button", { name: "Copy line", exact: true }).count() !== await rows.count()) throw new Error("Expand all skipped a log row");
			await page.getByRole("button", { name: "Collapse all", exact: true }).click();
			if (await page.getByRole("button", { name: "Copy line", exact: true }).count()) throw new Error("Collapse all retained expanded logs");
			await rows.first().click();
			break;
		}
		case "map-groups": {
			const nodes = page.locator(".svelte-flow__node-ownershipResource");
			const group = ownerlessStorageGroup(page);
			await group.waitFor();
			const before = await nodes.count();
			await group.click({ position: { x: 5, y: 5 } });
			await page.waitForFunction((count) => document.querySelectorAll(".svelte-flow__node-ownershipResource").length > count, before);
			await page.getByTitle("StorageClass/local-path", { exact: true }).waitFor();
			await group.click({ position: { x: 5, y: 5 } });
			await page.waitForFunction((count) => document.querySelectorAll(".svelte-flow__node-ownershipResource").length === count, before);
			break;
		}
		case "map-group-expand":
			await ownerlessStorageGroup(page).click({ position: { x: 5, y: 5 } });
			await page.getByTitle("StorageClass/local-path", { exact: true }).waitFor();
			break;
		case "map-group-collapsed":
			await page.getByTitle("StorageClass/local-path", { exact: true }).waitFor({ state: "detached" });
			break;
		case "map-focused":
			await page.locator(".svelte-flow").scrollIntoViewIfNeeded();
			await page.getByText(/^\d+ focused\s*\/\s*\d+ nodes\s*\/\s*\d+ edges$/).waitFor();
			await page.getByTitle("Deployment/payments-api", { exact: true }).waitFor();
			await page.getByTitle("ReplicaSet/payments-api-7d9c9b7f8d", { exact: true }).waitFor();
			await page.getByTitle("Pod/payments-api-7d9c9b7f8d-k2r9p", { exact: true }).waitFor();
			await page.getByTitle("Deployment/metrics-gateway", { exact: true }).waitFor({ state: "detached" });
			break;
		case "map-full":
		case "map-full-selection":
			await page.locator(".svelte-flow").scrollIntoViewIfNeeded();
			await page.getByText(/^\d+ nodes\s*\/\s*\d+ edges$/).waitFor();
			await page.locator(".svelte-flow__node-ownershipResource").first().waitFor();
			if (check === "map-full-selection") {
				await page.locator(".svelte-flow__controls-fitview").click();
				await page.waitForFunction(() => {
					const viewport = document.querySelector(".svelte-flow")?.getBoundingClientRect();
					return viewport && Array.from(document.querySelectorAll(".svelte-flow__node-ownershipResource")).some((node) => {
						const bounds = node.getBoundingClientRect();
						const id = node.getAttribute("data-id");
						return id !== null && !id.includes(":payments:") && bounds.right > viewport.left && bounds.left < viewport.right && bounds.bottom > viewport.top && bounds.top < viewport.bottom;
					});
				});
			}
			break;
		case "map-viewport": {
			// Name the wait that timed out: full-suite runs failed here with a bare 30 s timeout.
			const step = <T>(name: string, wait: Promise<T>) => wait.catch((error: unknown) => {
				throw new Error(`map-viewport ${name}: ${error instanceof Error ? error.message : String(error)}`);
			});
			await page.locator(".svelte-flow").scrollIntoViewIfNeeded();
			const viewport = page.locator(".svelte-flow__viewport");
			const node = page.locator(".svelte-flow__node-ownershipResource").first();
			const initialWidth = (await node.boundingBox())?.width;
			if (!initialWidth) throw new Error("Map nodes are missing");
			await page.locator(".svelte-flow__controls-zoomin").click();
			await step("zoom in", page.waitForFunction((before) => (document.querySelector(".svelte-flow__node-ownershipResource")?.getBoundingClientRect().width ?? 0) > before, initialWidth));
			const zoomedWidth = (await node.boundingBox())?.width;
			if (!zoomedWidth) throw new Error("Zoom removed map nodes");
			await page.locator(".svelte-flow__controls-zoomout").click();
			await step("zoom out", page.waitForFunction((before) => (document.querySelector(".svelte-flow__node-ownershipResource")?.getBoundingClientRect().width ?? before) < before, zoomedWidth));
			const pane = page.locator(".svelte-flow__pane");
			const bounds = await pane.boundingBox();
			if (!bounds) throw new Error("Map viewport unavailable");
			const beforePan = await viewport.getAttribute("style");
			const beforeNode = await node.boundingBox();
			await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
			await page.mouse.down();
			await page.mouse.move(bounds.x + bounds.width * 3 / 4, bounds.y + bounds.height * 3 / 4);
			await page.mouse.up();
			if (beforePan === await viewport.getAttribute("style")) throw new Error("Map drag did not pan");
			const afterNode = await node.boundingBox();
			if (!beforeNode || !afterNode || afterNode.x <= beforeNode.x || afterNode.y <= beforeNode.y) throw new Error("Map panned opposite the drag");
			const afterPan = await viewport.getAttribute("style");
			await page.locator(".svelte-flow__controls-fitview").click();
			await step("fit view moves", page.waitForFunction((before) => document.querySelector(".svelte-flow__viewport")?.getAttribute("style") !== before, afterPan));
			await step("fit view contains nodes", page.waitForFunction(() => {
				const pane = document.querySelector(".svelte-flow__pane")?.getBoundingClientRect();
				const nodes = document.querySelectorAll(".svelte-flow__node-ownershipResource");
				return pane && nodes.length > 0 && Array.from(nodes).every((node) => { const box = node.getBoundingClientRect(); return box.left >= pane.left && box.right <= pane.right && box.top >= pane.top && box.bottom <= pane.bottom; });
			}));
			break;
		}
		case "pagination": {
			const rows = page.getByRole("row").filter({ has: page.getByRole("button", { name: /^Open resource / }) });
			const identities = () => rows.allTextContents();
			await rows.first().waitFor();
			const total = Number((await page.getByText(/^\d+ total rows$/).textContent())?.split(" ")[0]);
			const checkGroups = async () => {
				for (const pattern of [/^\d+ resources on this page$/, /^\d+ on this page$/]) {
					const labels = await page.getByText(pattern).allTextContents();
					if (!labels.length || labels.reduce((sum, text) => sum + Number(text.trim().split(" ")[0]), 0) !== await rows.count()) throw new Error("Group totals do not match the rendered resources");
				}
			};
			await checkGroups();
			const first = await identities();
			await page.getByRole("button", { name: "Next", exact: true }).click();
			await page.getByText("Page 2 of 2", { exact: true }).waitFor();
			const second = await identities();
			await checkGroups();
			if (first.length + second.length !== total) throw new Error("Pager total disagrees with the two pages");
			if ([...first, ...second].some((name) => !name)) throw new Error("Resource identity is missing");
			if (new Set([...first, ...second]).size !== total) throw new Error("Pagination repeated a resource");
			if (!second.length || isDeepStrictEqual(first, second)) throw new Error("Next repeated page one");
			await page.getByRole("button", { name: "Previous", exact: true }).click();
			await page.getByText("Page 1 of 2", { exact: true }).waitFor();
			if (!isDeepStrictEqual(first, await identities())) throw new Error("Previous did not restore page one");
			break;
		}
		case "yaml-undo": {
			const editor = page.locator(".cm-content[contenteditable=true]");
			const lines = () => editor.locator(".cm-line").allTextContents();
			const original = await lines();
			await editor.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
			await page.keyboard.insertText("undo: probe");
			const edited = await lines();
			if (isDeepStrictEqual(edited, original)) throw new Error("The probe edit did not change the YAML");
			await editor.press(process.platform === "darwin" ? "Meta+z" : "Control+z");
			if (!isDeepStrictEqual(await lines(), original)) throw new Error("Undo did not restore the original YAML");
			await editor.press(process.platform === "darwin" ? "Meta+Shift+z" : "Control+y");
			if (!isDeepStrictEqual(await lines(), edited)) throw new Error("Redo did not restore the edited YAML");
			break;
		}
		case "yaml-editor": {
			const editor = page.locator(".cm-content[contenteditable=true]");
			await editor.fill("first: one\nsecond: two\n");
			await editor.press(process.platform === "darwin" ? "Meta+ArrowUp" : "Control+Home");
			await editor.press(process.platform === "darwin" ? "Meta+Shift+ArrowRight" : "Shift+End");
			await page.keyboard.insertText("first: replaced");
			if (!isDeepStrictEqual(await editor.locator(".cm-line").allTextContents(), ["first: replaced", "second: two", ""])) throw new Error("Selection replacement changed another YAML line");
			await editor.press(process.platform === "darwin" ? "Meta+ArrowDown" : "Control+End");
			await page.keyboard.insertText("third: three");
			if (!isDeepStrictEqual(await editor.locator(".cm-line").allTextContents(), ["first: replaced", "second: two", "third: three"])) throw new Error("End insertion changed earlier YAML lines");
			await page.locator(".cm-lineNumbers").waitFor();
			await page.locator(".cm-foldGutter").waitFor();
			break;
		}
		case "timeline": {
			const summary = page.locator("section").filter({ has: page.getByText("Incident summary", { exact: true }) }).last();
			await summary.getByText("Deployment/payments-api", { exact: true }).waitFor();
			const timeline = page.getByRole("region", { name: "Incident timeline" });
			const entries = timeline.locator("details");
			await entries.first().waitFor();
			await timeline.getByRole("button", { name: "Expand all", exact: true }).click();
			if (await timeline.locator("details:not([open])").count()) throw new Error("Expand all missed timeline entries");
			await timeline.getByRole("button", { name: "Collapse all", exact: true }).click();
			if (await timeline.locator("details[open]").count()) throw new Error("Collapse all retained timeline evidence");
			await entries.first().locator("summary").click();
			await entries.first().locator("[data-timeline-evidence]").waitFor();
			await entries.first().locator("summary").click();
			await entries.first().locator("[data-timeline-evidence]").waitFor({ state: "hidden" });
			break;
		}
		case "helm-resources": {
			await page.getByRole("button", { name: "View resources", exact: true }).last().click();
			await page.getByRole("textbox", { name: "Search resources" }).filter({ visible: true }).waitFor();
			const search = page.getByRole("textbox", { name: "Search resources" });
			if (await search.inputValue() !== "metrics-gateway") throw new Error("Helm did not preserve the chosen release");
			break;
		}
		default: throw new Error(`Unknown coverage check ${check}`);
	}
}

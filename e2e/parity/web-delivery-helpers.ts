import type { Locator, Page } from "playwright-core";

function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

/** A GitOps card or list row, by the label of its Details button. */
export function gitOpsDetails(page: Page, item: { kind: string; name: string; namespace: string }) {
	return page.getByRole("button", { name: `Open details for ${item.kind} ${item.name} in ${item.namespace}`, exact: true });
}
/** Scrolls virtual rows until the requested Application mounts or both ends are reached. */
export async function revealGitOps(page: Page, target: Locator) {
	await page.getByRole("heading", { level: 2 }).first().waitFor();
	for (const direction of [1, -1]) {
		while (await target.count() === 0) {
			const moved = await page.locator("[data-virtual-row]").first().evaluate(async (row, direction) => {
				const viewport = row.closest(".overflow-y-auto, .overflow-auto");
				if (!(viewport instanceof HTMLElement)) throw new Error("GitOps scroll viewport missing");
				const previous = viewport.scrollTop;
				viewport.scrollTop += direction * viewport.clientHeight;
				await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
				return viewport.scrollTop !== previous;
			}, direction);
			if (!moved) break;
		}
		if (await target.count()) return;
	}
	throw new Error("GitOps details target missing after scrolling the collection");
}
/** A Helm release card, by its title and namespace description. */
export function helmCard(page: Page, name: string, namespace: string) {
	return page.locator('[data-slot="card"]')
		.filter({ has: page.locator('[data-slot="card-title"]').getByText(name, { exact: true }) })
		.filter({ has: page.locator('[data-slot="card-description"]').getByText(namespace, { exact: true }) });
}
/** A revision in the Revisions list; its comparison picker shows the same label. */
export function revisionButton(page: Page, label: string) { return page.locator("button.w-full.p-3").filter({ has: page.locator("div.font-medium", { hasText: new RegExp(`^${literal(label)}$`) }) }); }

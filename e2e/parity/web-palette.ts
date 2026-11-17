import type { WebActions } from "./web-action";
import type { Page } from "playwright-core";
export const paletteActions = {
	async openPalette(page, step) {
		// The palette's window shortcut listener mounts with the header's search button.
		const button = page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true });
		await button.waitFor();
		if (step.via === "shortcut") await page.keyboard.press("ControlOrMeta+K");
		else await button.click();
		await page.getByRole("dialog").waitFor();
	},
	async searchPalette(page, step) {
		await page.getByPlaceholder("Search views, namespaces, and resources...").fill(step.value);
	},
	async expectPaletteResource(page, step) {
		await paletteResource(page, step).waitFor();
	},
	async expectNoPaletteResource(page, step) {
		if (await paletteResource(page, step).count()) throw new Error(`${step.name} is a palette result`);
	},
	async choosePaletteResource(page, step) {
		await paletteResource(page, step).click();
	},
	async expectPaletteEntry(page, step) {
		await page.getByRole("option", { name: step.label, exact: true }).waitFor();
	},
	async expectNoPaletteEntry(page, step) {
		if (await page.getByRole("option", { name: step.label, exact: true }).count()) throw new Error(`${step.label} is a palette result`);
	},
	async choosePaletteEntry(page, step) {
		await page.getByRole("option", { name: step.label, exact: true }).click();
	},
} satisfies Partial<WebActions>;

type Identity = { kind: string; name: string; namespace: string };
/** A palette resource result: its name, then cluster, namespace and kind. */
function paletteResource(page: Page, step: Identity) {
	const row = page.getByRole("option").filter({ has: page.locator("span.truncate", { hasText: new RegExp(`^${literal(step.name)}$`) }) }).filter({ has: page.getByText(step.kind, { exact: true }) });
	return step.namespace ? row.filter({ has: page.getByText(step.namespace, { exact: true }) }) : row;
}

function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

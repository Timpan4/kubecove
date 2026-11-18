import type { WebActions } from "./web-action";
import { tabTo } from "./web-keyboard";
import { sidebarRow } from "./web-sidebar-live";

export const navigationActions = {
	async openNamespaceList(page) {
		await page.getByRole("button", { name: "View all", exact: true }).click();
	},
	async expectNamespaceRow(page, step) {
		const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: step.name, exact: true }) });
		await row.getByRole("cell", { name: step.age, exact: true }).first().waitFor();
	},
	async resizeViewport(page, step) {
		await page.setViewportSize({ width: step.width, height: step.height });
	},
	async openNavigationDrawer(page) {
		await page.getByRole("button", { name: "Open workspace navigation", exact: true }).click();
	},
	async expectNavigationDrawer(page, step) {
		await page.getByRole("dialog", { name: "Workspace navigation", exact: true }).waitFor({ state: step.open ? "visible" : "hidden" });
	},
	async sidebarKey(page, step) {
		// Tab focuses the row without selecting it, so Enter and Space must select it themselves.
		await tabTo(page, sidebarRow(page, step.path));
		await page.keyboard.press(step.key);
	},
} satisfies Partial<WebActions>;

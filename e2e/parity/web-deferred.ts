import type { Page } from "playwright-core";

/**
 * The sidebar pages the app imports on first use (src/app/svelte/AppSurfaces.svelte), by tree
 * label. The bundled app serves each module under its source path (web-bundle-server.ts).
 */
export const DEFERRED_PAGES = new Map([
	["GitOps", "/src/features/gitops/GitOpsSurface.svelte"],
	["Helm", "/src/features/helm/HelmSurface.svelte"],
	["RBAC", "/src/features/rbac/RbacSurface.svelte"],
	["Incidents", "/src/features/incidents/IncidentSurface.svelte"],
	["Port Forwards", "/src/features/live-sessions/LiveSessionsSurface.svelte"],
]);

/**
 * Imports a page's module before the click that opens it, so the page mounts with the click.
 * Otherwise the import can outlast the steps that follow: a page that mounts after a scripted
 * read failure reads again on mount, and a scenario that counts requests sees a retry.
 */
export async function loadDeferredPage(page: Page, label: string) {
	const module = DEFERRED_PAGES.get(label);
	if (module) await page.evaluate(`import(${JSON.stringify(module)}).then(() => undefined)`);
}

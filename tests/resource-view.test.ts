import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

describe("resource view safeguards", () => {
	test("keeps the ownership map default-visible but unmountable", () => {
		const shellSource = readFileSync("src/app/svelte/WorkspaceShell.svelte", "utf8");
		const ownershipMapSource = readFileSync(
			"src/features/resources/OwnershipMap.svelte",
			"utf8",
		);
		const browserSource = readFileSync(
			"src/features/resources/ResourceBrowser.svelte",
			"utf8",
		);

		expect(browserSource).toContain("mapPanelOpen");
		expect(browserSource).toContain("tablePanelOpen");
		expect(browserSource).toContain(
			"enabled: Boolean(clusterContext && mapPanelOpen)",
		);
		expect(ownershipMapSource).toContain("aria-pressed={true}");
		expect(ownershipMapSource).toContain("onclick={onMapToggle}");
		expect(shellSource).toContain("selectedResource={focusedResource}");
		expect(browserSource).toContain(
			"grid-rows-[minmax(400px,1fr)_minmax(400px,1fr)]",
		);
		expect(browserSource).toContain(
			"xl:grid-cols-[minmax(420px,0.4fr)_minmax(620px,0.6fr)]",
		);
		expect(browserSource).toContain("min-h-[400px]");
		expect(browserSource).toContain("Collapse resource table");
		expect(browserSource).not.toContain('resourceView === "map"');
		expect(browserSource).not.toContain('resourceView === "table"');
	});
});

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

describe("resource view safeguards", () => {
	test("keeps the ownership map default-visible but unmountable", () => {
		const appSource = readFileSync("src/App.tsx", "utf8");
		const lazyViewsSource = readFileSync("src/app/lazyViews.ts", "utf8");
		const listSource = readFileSync(
			"src/features/resources/ResourceList.tsx",
			"utf8",
		);
		const layoutSource = readFileSync(
			"src/features/resources/ResourceMapTableLayout.tsx",
			"utf8",
		);
		const ownershipMapSource = readFileSync(
			"src/features/resources/OwnershipMap.tsx",
			"utf8",
		);

		expect(lazyViewsSource).toContain("lazy(() =>");
		expect(lazyViewsSource).toContain(
			'import("../features/resources/ResourceList")',
		);
		expect(listSource).toContain("<ResourceMapTableLayout");
		expect(listSource).toContain("mapPanelOpen");
		expect(listSource).toContain("setMapPanelOpen");
		expect(listSource).toContain(
			"enabled: Boolean(clusterContext && mapPanelOpen)",
		);
		expect(listSource).toContain(
			"onMapPanelOpenChange={handleMapPanelOpenChange}",
		);
		expect(layoutSource).toContain("tablePanelOpen");
		expect(layoutSource).toContain("mapPanelOpen");
		expect(layoutSource).toContain("LazyOwnershipMap");
		expect(layoutSource).toContain('import("./OwnershipMap")');
		expect(ownershipMapSource).toContain("aria-pressed={true}");
		expect(ownershipMapSource).toContain("onClick={onMapToggle}");
		expect(layoutSource).toContain("hasActiveSelection");
		expect(layoutSource).toContain(
			"Boolean(selectedResourceIdentityKey ?? selectedResourceKey)",
		);
		expect(layoutSource).toContain("mapHeightClassName");
		expect(appSource).toContain("selectedResource={selectedResource}");
		expect(listSource).toContain("activeSelectedResourceKey");
		expect(layoutSource).toContain(
			"grid-rows-[minmax(400px,1fr)_minmax(400px,1fr)]",
		);
		expect(layoutSource).toContain(
			"xl:grid-cols-[minmax(420px,0.4fr)_minmax(620px,0.6fr)]",
		);
		expect(layoutSource).toContain('mapHeightClassName = "h-full min-h-0"');
		expect(layoutSource).toContain("min-h-[400px]");
		expect(layoutSource).toContain("Collapse resource table");
		expect(listSource).not.toContain('resourceView === "map"');
		expect(listSource).not.toContain('resourceView === "table"');
	});
});

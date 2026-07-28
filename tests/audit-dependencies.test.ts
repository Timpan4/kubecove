import { expect, test } from "bun:test";

import { filterAuditReport } from "../scripts/audit-dependencies";

test("dependency audit ignores only the incompatible brace-expansion advisory", () => {
	const report = filterAuditReport({
		"brace-expansion": [
			{
				severity: "high",
				title: "Known incompatible advisory",
				url: "https://github.com/advisories/GHSA-mh99-v99m-4gvg",
			},
		],
		postcss: [
			{
				severity: "high",
				title: "Any other advisory",
				url: "https://github.com/advisories/GHSA-example",
			},
		],
	});

	expect(report).toEqual({
		postcss: [
			{
				severity: "high",
				title: "Any other advisory",
				url: "https://github.com/advisories/GHSA-example",
			},
		],
	});
});

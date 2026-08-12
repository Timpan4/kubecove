import { expect, test } from "bun:test";

import { filterAuditReport } from "../scripts/audit-dependencies";

const extractZipAdvisory = {
	severity: "high",
	title: "Known unpatched advisory",
	url: "https://github.com/advisories/GHSA-jmr9-qjv8-65gv",
};

test("dependency audit ignores only known dev-only transitive advisories", () => {
	const report = filterAuditReport(
		{
			"brace-expansion": [
				{
					severity: "high",
					title: "Known incompatible advisory",
					url: "https://github.com/advisories/GHSA-mh99-v99m-4gvg",
				},
			],
			"extract-zip": [extractZipAdvisory],
			postcss: [
				{
					severity: "high",
					title: "Any other advisory",
					url: "https://github.com/advisories/GHSA-example",
				},
			],
		},
		{},
	);

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

test("dependency audit restores an ignored production advisory", () => {
	expect(
		filterAuditReport(
			{ "extract-zip": [extractZipAdvisory] },
			{ "extract-zip": [extractZipAdvisory] },
		),
	).toEqual({ "extract-zip": [extractZipAdvisory] });
});

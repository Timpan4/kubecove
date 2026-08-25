type AuditAdvisory = {
	severity: string;
	title: string;
	url: string;
};

type AuditReport = Record<string, AuditAdvisory[]>;

const ignoredAdvisories = new Map([
	// TODO(Timpan4): Remove once @puppeteer/browsers no longer uses unpatched extract-zip 2.0.1.
	["GHSA-jmr9-qjv8-65gv", "extract-zip"],
	// TODO(Timpan4): Remove once WebdriverIO accepts brace-expansion 5.0.8+.
	["GHSA-mh99-v99m-4gvg", "brace-expansion"],
]);

function advisoryId(advisory: AuditAdvisory): string {
	return advisory.url.split("/").at(-1) ?? "";
}

export function filterAuditReport(
	report: AuditReport,
	productionReport: AuditReport,
): AuditReport {
	return Object.fromEntries(
		Object.entries(report).flatMap(([packageName, advisories]) => {
			const productionAdvisories = new Set(
				(productionReport[packageName] ?? []).map(advisoryId),
			);
			const remaining = advisories.filter((advisory) => {
				const id = advisoryId(advisory);
				return !(
					ignoredAdvisories.get(id) === packageName &&
					!productionAdvisories.has(id)
				);
			});
			return remaining.length > 0 ? [[packageName, remaining]] : [];
		}),
	);
}

async function runAudit(productionOnly: boolean): Promise<AuditReport> {
	const args = ["bun", "audit", "--json", "--audit-level=high"];
	if (productionOnly) args.push("--prod");
	const audit = Bun.spawn(args, { stderr: "pipe", stdout: "pipe" });
	const [stdout, stderr, exitCode] = await Promise.all([
		new Response(audit.stdout).text(),
		new Response(audit.stderr).text(),
		audit.exited,
	]);

	if (stderr) process.stderr.write(stderr);
	if (!stdout.trim()) process.exit(exitCode);
	try {
		// SAFETY: `bun audit --json` owns this versioned report payload.
		return JSON.parse(stdout) as AuditReport;
	} catch {
		process.stderr.write(stdout);
		process.exit(exitCode || 1);
	}
}

async function main(): Promise<void> {
	const [report, productionReport] = await Promise.all([
		runAudit(false),
		runAudit(true),
	]);
	const remaining = filterAuditReport(report, productionReport);
	if (Object.keys(remaining).length === 0) {
		console.log("No unignored vulnerabilities found");
		return;
	}

	for (const [packageName, advisories] of Object.entries(remaining)) {
		for (const advisory of advisories) {
			console.error(`${packageName}: ${advisory.severity}: ${advisory.title}`);
			console.error(advisory.url);
		}
	}
	process.exit(1);
}

if (import.meta.main) await main();

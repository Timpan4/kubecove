type AuditAdvisory = {
	severity: string;
	title: string;
	url: string;
};

type AuditReport = Record<string, AuditAdvisory[]>;

// TODO(Timpan4): Remove once WebdriverIO accepts brace-expansion 5.0.8+.
const ignoredAdvisories = new Set(["GHSA-mh99-v99m-4gvg"]);

export function filterAuditReport(report: AuditReport): AuditReport {
	return Object.fromEntries(
		Object.entries(report).flatMap(([packageName, advisories]) => {
			const remaining = advisories.filter(
				(advisory) => !ignoredAdvisories.has(advisory.url.split("/").at(-1) ?? ""),
			);
			return remaining.length > 0 ? [[packageName, remaining]] : [];
		}),
	);
}

async function main(): Promise<void> {
	const audit = Bun.spawn(["bun", "audit", "--json", "--audit-level=high"], {
		stderr: "pipe",
		stdout: "pipe",
	});
	const [stdout, stderr, exitCode] = await Promise.all([
		new Response(audit.stdout).text(),
		new Response(audit.stderr).text(),
		audit.exited,
	]);

	if (stderr) process.stderr.write(stderr);
	if (!stdout.trim()) process.exit(exitCode);

	let report: AuditReport;
	try {
		report = JSON.parse(stdout) as AuditReport;
	} catch {
		process.stderr.write(stdout);
		process.exit(exitCode || 1);
	}

	const remaining = filterAuditReport(report);
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

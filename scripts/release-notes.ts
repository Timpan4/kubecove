import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export type ChangelogSections = {
	added: string[];
	improved: string[];
	fixed: string[];
	release: string[];
};

export function buildChangelogSection(
	version: string,
	date: string,
	subjects: string[],
): string {
	const sections = classifyReleaseSubjects(subjects);
	const parts = [`## ${version} - ${date}`];

	appendCategory(parts, "Added", sections.added);
	appendCategory(parts, "Improved", sections.improved);
	appendCategory(parts, "Fixed", sections.fixed);
	appendCategory(parts, "Release", sections.release);

	if (parts.length === 1) {
		parts.push("### Release", "", "- Prepared KubeCove beta release metadata.");
	}

	return `${parts.join("\n")}\n`;
}

export function classifyReleaseSubjects(subjects: string[]): ChangelogSections {
	const sections: ChangelogSections = { added: [], improved: [], fixed: [], release: [] };
	const seen = new Set<string>();

	for (const subject of subjects) {
		const category = categoryForSubject(subject);
		if (!category) continue;
		const bullet = normalizeSubject(subject);
		if (!bullet || seen.has(bullet)) continue;
		seen.add(bullet);
		sections[category].push(bullet);
	}

	return sections;
}

export function upsertChangelogSection(
	changelog: string,
	version: string,
	section: string,
): string {
	const header = `## ${version} - `;
	const lines = changelog.split(/\r?\n/);
	const firstVersionIndex = lines.findIndex((line) => /^## \d+\.\d+\.\d+/.test(line));
	const existingIndex = lines.findIndex((line) => line.startsWith(header));

	if (existingIndex !== -1) {
		let endIndex = lines.length;
		for (let index = existingIndex + 1; index < lines.length; index += 1) {
			if (/^## \d+\.\d+\.\d+/.test(lines[index] ?? "")) {
				endIndex = index;
				break;
			}
		}
		return [
			...lines.slice(0, existingIndex),
			...section.trimEnd().split("\n"),
			"",
			...lines.slice(endIndex).filter((line, index) => index !== 0 || line !== ""),
		].join("\n");
	}

	if (firstVersionIndex === -1) return `${changelog.trimEnd()}\n\n${section}\n`;

	return [
		...lines.slice(0, firstVersionIndex),
		section.trimEnd(),
		"",
		...lines.slice(firstVersionIndex),
	].join("\n");
}

export function updateReleaseDocsVersion(version: string, root = "."): void {
	for (const path of ["README.md", "docs/release.md"]) {
		const fullPath = join(root, path);
		const next = readFileSync(fullPath, "utf8").replace(
			/Current version metadata: `[^`]+`\./,
			`Current version metadata: \`${version}\`.`,
		);
		writeFileSync(fullPath, next);
	}
}

export function updateChangelog(
	version: string,
	date: string,
	subjects: string[],
	root = ".",
): void {
	const changelogPath = join(root, "CHANGELOG.md");
	const changelog = readFileSync(changelogPath, "utf8");
	const section = buildChangelogSection(version, date, subjects);
	writeFileSync(changelogPath, upsertChangelogSection(changelog, version, section));
}

export function readChangelogReleaseBody(version: string, root = "."): string {
	const changelog = readFileSync(join(root, "CHANGELOG.md"), "utf8");
	const lines = changelog.split(/\r?\n/);
	const startIndex = lines.findIndex((line) => line.startsWith(`## ${version} - `));
	if (startIndex === -1) throw new Error(`CHANGELOG.md is missing ${version}.`);

	let endIndex = lines.length;
	for (let index = startIndex + 1; index < lines.length; index += 1) {
		if (/^## \d+\.\d+\.\d+/.test(lines[index] ?? "")) {
			endIndex = index;
			break;
		}
	}

	return lines.slice(startIndex + 1, endIndex).join("\n").trim();
}

function appendCategory(parts: string[], heading: string, items: string[]): void {
	if (items.length === 0) return;
	parts.push(`### ${heading}`, "", ...items.map((item) => `- ${item}`), "");
}

function categoryForSubject(subject: string): keyof ChangelogSections | null {
	const lower = subject.toLowerCase();
	if (/^(✨\s*)?(feat)(\(.+\))?:?\s/.test(lower) || subject.startsWith("✨")) return "added";
	if (/^(🐛\s*)?(fix)(\(.+\))?:?\s/.test(lower) || subject.startsWith("🐛")) return "fixed";
	if (
		/^(🚀\s*)?(perf)(\(.+\))?:?\s/.test(lower) ||
		/^(♻️\s*)?(refactor)(\(.+\))?:?\s/.test(lower) ||
		/^(🎨\s*)?(style)(\(.+\))?:?\s/.test(lower) ||
		["🚀", "♻️", "🎨"].some((prefix) => subject.startsWith(prefix))
	) return "improved";
	if (/^(🔧\s*)?(chore|release)(\(.+\))?:?\s/.test(lower) || subject.startsWith("🔧")) {
		return lower.includes("release") || lower.includes("prepare kubecove") ? "release" : null;
	}
	if (lower.startsWith("improve ") || lower.startsWith("keep ")) return "improved";
	return null;
}

function normalizeSubject(subject: string): string {
	return subject
		.replace(/^(?:✨|🐛|🚀|♻️|🎨|🔧|✅)\s*/u, "")
		.replace(/^(feat|fix|perf|refactor|style|chore|release)(\([^)]+\))?:\s*/i, "")
		.replace(/^(feat|fix|perf|refactor|style|chore|release)\s+/i, "")
		.trim()
		.replace(/\.$/, "");
}

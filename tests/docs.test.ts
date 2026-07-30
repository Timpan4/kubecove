import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "bun:test";

const root = resolve(import.meta.dir, "..");
const readme = readFileSync(resolve(root, "README.md"), "utf8");
const workflow = readFileSync(resolve(root, ".github/workflows/publish-wiki.yml"), "utf8");
const issueForm = readFileSync(resolve(root, ".github/ISSUE_TEMPLATE/documentation.yml"), "utf8");

test("README exposes the primary product and discovery terms", () => {
	expect(readme).toMatch(/local Kubernetes desktop (?:UI|GUI)/i);
	expect(readme).toMatch(/Kubernetes GUI/i);
	expect(readme).toMatch(/cluster inspection/i);
	expect(readme).toMatch(/Kubernetes troubleshooting/i);
	expect(readme).toMatch(/Argo CD/i);
	expect(readme).toMatch(/Flux/i);
	expect(readme).toMatch(/Helm/i);
	expect(readme).toMatch(/guarded cluster operations/i);
	expect(readme).toContain("https://github.com/Timpan4/kubecove/releases/latest");
	expect(readme).toContain("https://github.com/Timpan4/kubecove/wiki");
});

test("README screenshots use descriptive alt text", () => {
	const altTexts = [...readme.matchAll(/!\[([^\]]+)\]\(docs\/assets\/[^\n)]+\)/g)].map((match) => match[1]);

	expect(altTexts.length).toBeGreaterThanOrEqual(3);
	expect(altTexts.every((alt) => alt.length >= 40)).toBe(true);
	expect(altTexts.some((alt) => /Kubernetes desktop UI/i.test(alt))).toBe(true);
});

test("Wiki publishing workflow syncs only canonical Wiki source", () => {
	expect(workflow).toMatch(/branches:\s*\n\s*- main/);
	expect(workflow).toMatch(/paths:\s*\n\s*- docs\/wiki\/\*\*/);
	expect(workflow).toContain("workflow_dispatch:");
	expect(workflow).toMatch(/permissions:\s*\n\s*contents: write/);
	expect(workflow).toContain("concurrency:");
	expect(workflow).toContain("actions/checkout@v7");
	expect(workflow).toContain("GITHUB_TOKEN");
	expect(workflow).toContain("github.com/Timpan4/kubecove.wiki.git");
	expect(workflow).toContain("rsync -a --delete --exclude=.git docs/wiki/ wiki/");
	expect(workflow).toContain("diff -qr --exclude=.git docs/wiki wiki");
	expect(workflow).toContain("git -C wiki status --porcelain");
	expect(workflow).toContain("git -C wiki push");
	expect(workflow).not.toContain("--force");
	expect(workflow).not.toMatch(/wiki-page-publish|github-wiki-action/i);
});

test("documentation issue form gathers corrections without sensitive data", () => {
	expect(issueForm).toMatch(/id: page_url(?:(?!\n\s*-\s*type:)[\s\S])*?required: true/);
	expect(issueForm).toMatch(/id: problem(?:(?!\n\s*-\s*type:)[\s\S])*?required: true/);
	expect(issueForm).toMatch(/id: expected_correction(?:(?!\n\s*-\s*type:)[\s\S])*?required: true/);
	expect(issueForm).toMatch(/id: app_version(?:(?!\n\s*-\s*type:)[\s\S])*?required: false/);
	expect(issueForm).toMatch(/id: operating_system(?:(?!\n\s*-\s*type:)[\s\S])*?required: false/);
	expect(issueForm).toContain(
		"Do not include secrets, credentials, kubeconfig contents, tokens, customer data, or sensitive diagnostics.",
	);
	expect(issueForm).toContain("SECURITY.md");
});

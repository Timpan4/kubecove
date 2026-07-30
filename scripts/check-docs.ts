import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, relative, resolve } from "node:path";

type Source = { path: string; text: string };
type Link = { target: string; anchor: string; index: number };

const wikiUrl = /^https:\/\/github\.com\/Timpan4\/kubecove\/wiki(?:\/([^/?#]+))?(?:[?#].*)?$/;
const assetUrl = /^https:\/\/raw\.githubusercontent\.com\/Timpan4\/kubecove\/main\/docs\/assets\/(.+?)(?:[?#].*)?$/;

function files(directory: string): string[] {
	if (!existsSync(directory)) return [];
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = resolve(directory, entry.name);
		return entry.isDirectory() ? files(path) : [path];
	});
}

function lineAt(text: string, index: number): number {
	return text.slice(0, index).split("\n").length;
}

function withoutFences(text: string): string {
	return text.replace(/^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1[ \t]*$/gm, (fence) =>
		fence.replace(/[^\n]/g, " "),
	);
}

function slug(heading: string): string {
	return heading
		.toLowerCase()
		.replace(/\[[^\]]*]\(([^)]*)\)/g, "$1")
		.replace(/[^\w\s-]/g, "")
		.trim()
		.replace(/\s+/g, "-");
}

function anchors(text: string): Set<string> {
	const found = new Set<string>();
	const counts = new Map<string, number>();
	for (const match of withoutFences(text).matchAll(/^#{1,6}\s+(.+?)\s*#*\s*$/gm)) {
		const base = slug(match[1]);
		const count = counts.get(base) ?? 0;
		counts.set(base, count + 1);
		found.add(count ? `${base}-${count}` : base);
	}
	return found;
}

function links(text: string): Link[] {
	const found: Link[] = [];
	const markdownLink = /!?\[[^\]]*]\(\s*(?:<([^>\n]+)>|([^\s)]+))/g;
	const markdown = withoutFences(text);
	for (const match of markdown.matchAll(markdownLink)) {
		const raw = (match[1] ?? match[2]).replace(/\\([()[\]])/g, "$1");
		const [target, anchor = ""] = raw.split("#", 2);
		found.push({ target, anchor: decodeURIComponent(anchor), index: match.index ?? 0 });
	}
	return found;
}

function decoded(path: string): string {
	try {
		return decodeURIComponent(path);
	} catch {
		return path;
	}
}

function targetPath(root: string, source: string, target: string): string | undefined {
	if (!target) return source;
	const wiki = target.match(wikiUrl);
	if (wiki) return resolve(root, "docs/wiki", `${decoded(wiki[1] || "Home")}.md`);
	const asset = target.match(assetUrl);
	if (asset) return resolve(root, "docs/assets", decoded(asset[1]));
	if (!target || target.startsWith("//") || /^[a-z][a-z\d+.-]*:/i.test(target)) return undefined;

	const local = resolve(dirname(source), decoded(target.split("?", 1)[0]));
	if (!extname(local) && dirname(source) === resolve(root, "docs/wiki") && existsSync(`${local}.md`)) return `${local}.md`;
	return local;
}

export function checkDocs(root = process.cwd()): string[] {
	const errors: string[] = [];
	const required = ["README.md", "SECURITY.md", "CONTRIBUTING.md"].map((file) => resolve(root, file));
	const markdown = [...required, ...files(resolve(root, "docs")).filter((file) => file.endsWith(".md"))];
	const sources: Source[] = [];

	for (const path of markdown) {
		if (!existsSync(path)) {
			errors.push(`${relative(root, path)}: required documentation file is missing`);
			continue;
		}
		sources.push({ path, text: readFileSync(path, "utf8") });
	}

	const sourceByPath = new Map(sources.map((source) => [source.path, source]));
	const assetReferences = new Set<string>();
	const missingTargets = new Set<string>();
	const sidebarReferences = new Map<string, number>();
	const wikiDirectory = resolve(root, "docs/wiki");
	const sidebar = resolve(wikiDirectory, "_Sidebar.md");

	for (const source of sources) {
		const sourceLabel = relative(root, source.path);
		const h1s = [...withoutFences(source.text).matchAll(/^#\s+.+$/gm)];
		if (source.path.startsWith(`${wikiDirectory}/`) && h1s.length !== 1) {
			errors.push(`${sourceLabel}: expected exactly one H1, found ${h1s.length}`);
		}
		const versionedPage = source.path === resolve(root, "README.md") || source.path.startsWith(`${wikiDirectory}/`);
		const currentSource = source.text.match(/\bcurrent[- ]source\b/i);
		if (versionedPage && currentSource) {
			errors.push(`${sourceLabel}:${lineAt(source.text, currentSource.index ?? 0)}: remove "current source" wording`);
		}
		const version = source.text.match(
			/\bapp-v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?\b|\bKubeCove(?:\s+(?:app|version)){0,2}\s+v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?\b|\b(?:current (?:app )?version|app version)(?: metadata)?\s*:?\s*`?v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?\b/i,
		);
		if (versionedPage && version) {
			errors.push(`${sourceLabel}:${lineAt(source.text, version.index ?? 0)}: remove KubeCove app version literal`);
		}

		for (const link of links(source.text)) {
			const path = targetPath(root, source.path, link.target);
			if (!path) continue;
			const label = `${sourceLabel}:${lineAt(source.text, link.index)}`;
			if (relative(root, path).startsWith("..")) {
				errors.push(`${label}: local link escapes repository ${link.target}`);
				continue;
			}
			if (!existsSync(path)) {
				if (!missingTargets.has(path)) {
					missingTargets.add(path);
					errors.push(`${label}: missing link target ${link.target}`);
				}
				continue;
			}
			if (link.anchor) {
				const target = sourceByPath.get(path);
				if (target && !anchors(target.text).has(slug(link.anchor))) {
					errors.push(`${label}: missing anchor #${link.anchor} in ${relative(root, path)}`);
				}
			}
			if (path.startsWith(`${resolve(root, "docs/assets/wiki")}/`)) {
				assetReferences.add(relative(root, path));
			}
			if (source.path === sidebar && path.startsWith(`${wikiDirectory}/`) && path.endsWith(".md") && path !== sidebar) {
				sidebarReferences.set(path, (sidebarReferences.get(path) ?? 0) + 1);
			}
		}
	}

	const mermaidPages = new Set([
		resolve(wikiDirectory, "Safety-Data-and-Architecture.md"),
		resolve(wikiDirectory, "Guarded-Operations.md"),
		resolve(wikiDirectory, "GitOps-Argo-CD-and-Flux.md"),
	]);
	let mermaidCount = 0;
	const mermaidByPage = new Map<string, number>();
	for (const source of sources) {
		const count = [...source.text.matchAll(/^```mermaid\s*$/gim)].length;
		mermaidCount += count;
		if (mermaidPages.has(source.path)) mermaidByPage.set(source.path, count);
		if (count && !mermaidPages.has(source.path)) {
			errors.push(`${relative(root, source.path)}: Mermaid is only allowed in designated Wiki pages`);
		}
	}
	for (const page of mermaidPages) {
		const count = mermaidByPage.get(page) ?? 0;
		if (count !== 1) errors.push(`${relative(root, page)}: expected one Mermaid fence, found ${count}`);
	}
	if (mermaidCount !== 3) errors.push(`docs: expected 3 Mermaid fences, found ${mermaidCount}`);

	const wikiFiles = files(wikiDirectory).filter((file) => file.endsWith(".md"));
	const nested = wikiFiles.filter((file) => dirname(file) !== wikiDirectory);
	if (nested.length) errors.push(`docs/wiki: must be flat; nested page ${relative(wikiDirectory, nested[0])}`);
	const pages = wikiFiles.filter((file) => file !== sidebar);
	if (pages.length !== 18) errors.push(`docs/wiki: expected 18 user pages, found ${pages.length}`);
	if (!existsSync(sidebar)) errors.push("docs/wiki: missing _Sidebar.md");
	if (existsSync(sidebar)) for (const page of pages) {
		const count = sidebarReferences.get(page) ?? 0;
		if (count !== 1) errors.push(`docs/wiki/_Sidebar.md: expected one link to ${relative(wikiDirectory, page)}, found ${count}`);
	}

	for (const asset of files(resolve(root, "docs/assets/wiki"))) {
		if (statSync(asset).isFile() && !assetReferences.has(relative(root, asset))) {
			errors.push(`${relative(root, asset)}: screenshot is not referenced`);
		}
	}

	return errors;
}

const errors = checkDocs();
if (errors.length) {
	console.error(errors.join("\n"));
	process.exitCode = 1;
}

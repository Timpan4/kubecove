import { yamlLanguage } from "@codemirror/lang-yaml";
import { highlightTree, tagHighlighter, tags } from "@lezer/highlight";
import { stringify } from "yaml";
import type { UnifiedDiffLine } from "@/features/resource-detail/yamlTabDiff";
import type {
	ArgoManagedResource,
	ArgoResourceComparison,
	JsonValue,
} from "@/lib/types";
import { argoComparisonDocument, argoResourceIdentityKey } from "./argo-workspace-model";

export const COMPACT_ARGO_DIFF_LINES = 24;

export type YamlDiffSegment = { text: string; className: string };

export function argoDocumentText(value: JsonValue | undefined): string {
	if (isString(value)) return value.trimEnd();
	return value === undefined || value === null
		? ""
		: stringify(value).trimEnd();
}

function hasArgoDocument(value: JsonValue | undefined): boolean {
	return isString(value) ? value.trimEnd() !== "" : value != null;
}

function isString(value: JsonValue | undefined): value is string {
	return Object(value) !== value && String(value) === value;
}

/** `comparisonsByKey` comes from `argoComparisonIndex` over the snapshot's comparisons. */
export function resolveArgoResourceComparison(
	resource: ArgoManagedResource,
	comparisonsByKey: ReadonlyMap<string, ArgoResourceComparison>,
	transport: "connected" | "kubernetes" | undefined,
	selectedResourceKey: string | null,
	selectedComparison: ArgoResourceComparison | null | undefined,
): ArgoResourceComparison | null {
	const key = argoResourceIdentityKey(resource);
	if (
		transport === "connected" &&
		selectedResourceKey === key &&
		selectedComparison &&
		argoResourceIdentityKey(selectedComparison.resource) === selectedResourceKey
	) {
		return selectedComparison;
	}
	return key === null ? null : (comparisonsByKey.get(key) ?? null);
}

export function projectArgoResourceComparison(
	resource: ArgoManagedResource,
	comparison: ArgoResourceComparison | null,
) {
	const documents = argoComparisonDocument(resource, comparison);
	let desired: string | undefined;
	let live: string | undefined;
	let normalizedLive: string | undefined;
	return {
		get desired() {
			desired ??= argoDocumentText(documents.desired);
			return desired;
		},
		get live() {
			live ??= argoDocumentText(documents.live);
			return live;
		},
		get normalizedLive(): string {
			if (documents.normalizedLive === documents.live) return this.live;
			normalizedLive ??= argoDocumentText(documents.normalizedLive);
			return normalizedLive;
		},
		exact: documents.exact,
		get hasDiff() {
			return hasArgoDocument(documents.desired) && hasArgoDocument(documents.normalizedLive);
		},
		get added() {
			return !hasArgoDocument(documents.live) && hasArgoDocument(documents.desired);
		},
		get modified(): boolean {
			if (documents.modified != null) return documents.modified;
			if (documents.desired === documents.normalizedLive) return false;
			const hasDesired = hasArgoDocument(documents.desired);
			const hasNormalizedLive = hasArgoDocument(documents.normalizedLive);
			return hasDesired && hasNormalizedLive
				? this.desired !== this.normalizedLive
				: hasDesired !== hasNormalizedLive;
		},
	};
}

const yamlDiffHighlighter = tagHighlighter([
	{ tag: tags.keyword, class: "text-blue-700 dark:text-blue-300" },
	{
		tag: [tags.atom, tags.bool],
		class: "text-violet-700 dark:text-violet-300",
	},
	{ tag: tags.number, class: "text-amber-700 dark:text-amber-300" },
	{
		tag: [tags.string, tags.content],
		class: "text-emerald-700 dark:text-emerald-300",
	},
	{
		tag: [tags.propertyName, tags.definition(tags.propertyName)],
		class: "text-sky-700 dark:text-sky-300",
	},
	{ tag: tags.comment, class: "text-muted-foreground italic" },
	{ tag: tags.punctuation, class: "text-muted-foreground" },
	{ tag: tags.operator, class: "text-rose-700 dark:text-rose-300" },
	{ tag: tags.invalid, class: "text-destructive" },
]);

export function highlightedArgoDiffSegments(
	line: UnifiedDiffLine,
): YamlDiffSegment[] {
	if (
		line.type !== "add" &&
		line.type !== "remove" &&
		line.type !== "context"
	) {
		return [{ text: line.text, className: "" }];
	}
	const prefix = line.text.slice(0, 1);
	const source = line.text.slice(1);
	const segments: YamlDiffSegment[] = [
		{ text: prefix, className: "font-semibold" },
	];
	let cursor = 0;
	highlightTree(
		yamlLanguage.parser.parse(source),
		yamlDiffHighlighter,
		(from, to, className) => {
			if (from > cursor)
				segments.push({ text: source.slice(cursor, from), className: "" });
			segments.push({ text: source.slice(from, to), className });
			cursor = to;
		},
	);
	if (cursor < source.length)
		segments.push({ text: source.slice(cursor), className: "" });
	return segments;
}

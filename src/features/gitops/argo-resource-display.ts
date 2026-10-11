import { yamlLanguage } from "@codemirror/lang-yaml";
import { highlightTree, tagHighlighter, tags } from "@lezer/highlight";
import { stringify } from "yaml";
import type { UnifiedDiffLine } from "@/features/resource-detail/yamlTabDiff";
import type {
	ArgoManagedResource,
	ArgoResourceComparison,
	JsonValue,
} from "@/lib/types";
import {
	argoComparisonForResource,
	argoResourceIdentityKey,
} from "./argo-workspace-model";

export type YamlDiffSegment = { text: string; className: string };

export function argoDocumentText(value: JsonValue | undefined): string {
	if (isString(value)) return value.trimEnd();
	return value === undefined || value === null
		? ""
		: stringify(value).trimEnd();
}

export function resolveArgoResourceComparison(
	resource: ArgoManagedResource,
	comparisons: ArgoResourceComparison[],
	transport: "connected" | "kubernetes" | undefined,
	selectedResourceKey: string | null,
	selectedComparison: ArgoResourceComparison | null | undefined,
): ArgoResourceComparison | null {
	const eager = argoComparisonForResource(comparisons, resource);
	if (
		transport === "connected" &&
		selectedResourceKey === argoResourceIdentityKey(resource) &&
		selectedComparison &&
		argoResourceIdentityKey(selectedComparison.resource) === selectedResourceKey
	) {
		return selectedComparison;
	}
	return eager;
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

function isString(value: JsonValue | undefined): value is string {
	return String(value) === value;
}

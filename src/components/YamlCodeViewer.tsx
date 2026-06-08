import CodeMirror from "@uiw/react-codemirror";
import { yaml } from "@codemirror/lang-yaml";
import {
	HighlightStyle,
	syntaxHighlighting,
} from "@codemirror/language";
import {
	linter,
	lintGutter,
	lintKeymap,
	type Diagnostic,
} from "@codemirror/lint";
import { RangeSetBuilder, StateField } from "@codemirror/state";
import {
	Decoration,
	type DecorationSet,
	EditorView,
	keymap,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { useMemo } from "react";
import { parse, parseDocument, stringify, type YAMLError } from "yaml";
import type { YamlEncoding } from "@/lib/types";

interface YamlCodeViewerProps {
	value: string;
	editable?: boolean;
	onChange?: (value: string) => void;
	minHeight?: string;
	extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
}

const YAML_PARSE_OPTIONS = {
	prettyErrors: false,
	strict: true,
	uniqueKeys: true,
} as const;

const YAML_FORMAT_OPTIONS = {
	indent: 2,
	lineWidth: 0,
} as const;

const yamlEditorTheme = EditorView.theme(
	{
		"&": {
			backgroundColor: "var(--card)",
			color: "var(--foreground)",
			fontSize: "12px",
		},
		".cm-editor": {
			backgroundColor: "var(--card)",
		},
		".cm-scroller": {
			backgroundColor: "var(--card)",
			fontFamily:
				'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
			lineHeight: "1.55",
			letterSpacing: "0.015em",
		},
		".cm-content": {
			caretColor: "var(--primary)",
			padding: "10px 0",
		},
		".cm-line": {
			padding: "0 14px",
		},
		".kubecove-yaml-indent": {
			backgroundColor:
				"color-mix(in oklch, var(--kubecove-yaml-indent-color) 11%, transparent)",
			backgroundImage:
				"radial-gradient(circle at 0.5ch 50%, color-mix(in oklch, var(--muted-foreground) 58%, transparent) 0 1px, transparent 1.3px)",
			backgroundPosition: "0 0",
			backgroundRepeat: "repeat-x",
			backgroundSize: "1ch 100%",
		},
		".kubecove-yaml-indent-0": {
			"--kubecove-yaml-indent-color": "oklch(0.72 0.14 28)",
		},
		".kubecove-yaml-indent-1": {
			"--kubecove-yaml-indent-color": "oklch(0.79 0.13 78)",
		},
		".kubecove-yaml-indent-2": {
			"--kubecove-yaml-indent-color": "oklch(0.76 0.12 145)",
		},
		".kubecove-yaml-indent-3": {
			"--kubecove-yaml-indent-color": "oklch(0.74 0.12 210)",
		},
		".kubecove-yaml-indent-4": {
			"--kubecove-yaml-indent-color": "oklch(0.75 0.12 285)",
		},
		".kubecove-yaml-indent-5": {
			"--kubecove-yaml-indent-color": "oklch(0.78 0.12 335)",
		},
		".cm-gutters": {
			backgroundColor: "color-mix(in oklch, var(--card) 88%, black)",
			borderRight: "1px solid var(--border)",
			color: "var(--muted-foreground)",
		},
		".cm-lineNumbers .cm-gutterElement": {
			minWidth: "34px",
			padding: "0 10px 0 8px",
		},
		".cm-foldGutter .cm-gutterElement": {
			padding: "0 6px 0 2px",
		},
		".cm-activeLine": {
			backgroundColor: "color-mix(in oklch, var(--muted) 55%, transparent)",
		},
		".cm-activeLineGutter": {
			backgroundColor: "color-mix(in oklch, var(--muted) 65%, transparent)",
			color: "var(--foreground)",
		},
		".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
			backgroundColor:
				"color-mix(in oklch, var(--primary) 62%, transparent)",
		},
		".cm-selectionMatch": {
			backgroundColor:
				"color-mix(in oklch, var(--primary) 27%, transparent)",
			outline: "1px solid color-mix(in oklch, var(--primary) 34%, transparent)",
		},
		"&.cm-focused": {
			outline: "1px solid color-mix(in oklch, var(--primary) 45%, transparent)",
		},
		".cm-cursor": {
			borderLeftColor: "var(--primary)",
		},
		".cm-foldPlaceholder": {
			backgroundColor: "var(--muted)",
			borderColor: "var(--border)",
			color: "var(--muted-foreground)",
		},
		".cm-lintRange-error": {
			backgroundImage:
				"linear-gradient(135deg, transparent 45%, var(--destructive) 45%, var(--destructive) 55%, transparent 55%)",
			backgroundPosition: "left bottom",
			backgroundRepeat: "repeat-x",
			backgroundSize: "6px 3px",
		},
		".cm-lintRange-warning": {
			backgroundImage:
				"linear-gradient(135deg, transparent 45%, oklch(0.82 0.12 78) 45%, oklch(0.82 0.12 78) 55%, transparent 55%)",
			backgroundPosition: "left bottom",
			backgroundRepeat: "repeat-x",
			backgroundSize: "6px 3px",
		},
		".cm-diagnostic": {
			backgroundColor: "var(--popover)",
			border: "1px solid var(--border)",
			color: "var(--popover-foreground)",
			fontFamily: "inherit",
			fontSize: "12px",
		},
		".kubecove-yaml-string": {
			color: "oklch(0.83 0.08 145) !important",
		},
		".kubecove-yaml-number": {
			color: "oklch(0.82 0.12 78) !important",
		},
		".kubecove-yaml-bool": {
			color: "oklch(0.8 0.12 305) !important",
		},
	},
	{ dark: true },
);

const yamlHighlightStyle = HighlightStyle.define([
	{ tag: tags.keyword, color: "oklch(0.78 0.12 250)" },
	{ tag: [tags.atom, tags.bool], color: "oklch(0.8 0.12 305)" },
	{ tag: tags.number, color: "oklch(0.82 0.12 78)" },
	{ tag: tags.string, color: "oklch(0.82 0.08 145)" },
	{ tag: tags.propertyName, color: "oklch(0.8 0.11 230)" },
	{ tag: tags.definition(tags.propertyName), color: "oklch(0.84 0.1 225)" },
	{ tag: tags.comment, color: "var(--muted-foreground)" },
	{ tag: tags.punctuation, color: "oklch(0.78 0.01 90)" },
	{ tag: tags.operator, color: "oklch(0.78 0.08 20)" },
	{ tag: tags.invalid, color: "var(--destructive)" },
]);

const yamlIndentDepthClassCount = 6;

export function formatYamlDocument(
	value: string,
	encoding: YamlEncoding = "yaml",
): string {
	const document = parseDocument(value, YAML_PARSE_OPTIONS);
	if (document.errors.length > 0) {
		throw new Error(document.errors[0]?.message ?? "YAML parse failed.");
	}
	if (encoding === "kyaml") {
		return `${formatKyamlValue(parse(value), 0)}\n`;
	}
	return stringify(parse(value), YAML_FORMAT_OPTIONS);
}

function formatKyamlValue(value: unknown, indent: number): string {
	if (value === null || value === undefined) return "null";
	if (typeof value === "boolean" || typeof value === "number") return String(value);
	if (typeof value === "string") return JSON.stringify(value);
	if (Array.isArray(value)) return formatKyamlArray(value, indent);
	if (typeof value === "object") {
		return formatKyamlObject(value as Record<string, unknown>, indent);
	}
	return JSON.stringify(String(value));
}

function formatKyamlArray(values: unknown[], indent: number): string {
	if (values.length === 0) return "[]";
	const childIndent = indent + 2;
	const lines = values.map(
		(value) =>
			`${" ".repeat(childIndent)}${formatKyamlValue(value, childIndent)},`,
	);
	return `[\n${lines.join("\n")}\n${" ".repeat(indent)}]`;
}

function formatKyamlObject(value: Record<string, unknown>, indent: number): string {
	const entries = Object.entries(value);
	if (entries.length === 0) return "{}";
	const childIndent = indent + 2;
	const lines = entries.map(
		([key, child]) =>
			`${" ".repeat(childIndent)}${formatKyamlKey(key)}: ${formatKyamlValue(child, childIndent)},`,
	);
	return `{\n${lines.join("\n")}\n${" ".repeat(indent)}}`;
}

function formatKyamlKey(key: string): string {
	return /^[A-Za-z_][A-Za-z0-9_.\/-]*$/.test(key)
		? key
		: JSON.stringify(key);
}

function yamlDiagnostics(value: string): Diagnostic[] {
	if (value.trim().length === 0) return [];
	try {
		const document = parseDocument(value, YAML_PARSE_OPTIONS);
		return [...document.errors, ...document.warnings].map((error) =>
			yamlErrorDiagnostic(error, value.length),
		);
	} catch (error) {
		return [yamlErrorDiagnostic(error, value.length)];
	}
}

function yamlErrorDiagnostic(error: unknown, documentLength: number): Diagnostic {
	const yamlError = error as Partial<YAMLError>;
	const from = clampOffset(yamlError.pos?.[0] ?? 0, documentLength);
	const rawTo = clampOffset(yamlError.pos?.[1] ?? from + 1, documentLength);
	const to =
		rawTo > from ? rawTo : documentLength > from ? Math.min(from + 1, documentLength) : from;

	return {
		from,
		to,
		severity: yamlError.name === "YAMLWarning" ? "warning" : "error",
		source: "YAML",
		message:
			typeof yamlError.message === "string" && yamlError.message.length > 0
				? yamlError.message
				: "Invalid YAML.",
	};
}

function clampOffset(offset: number, documentLength: number): number {
	if (!Number.isFinite(offset)) return 0;
	return Math.min(Math.max(0, offset), documentLength);
}

const yamlValueDecorations = StateField.define<DecorationSet>({
	create(state) {
		return buildYamlValueDecorations(state);
	},
	update(decorations, transaction) {
		return transaction.docChanged
			? buildYamlValueDecorations(transaction.state)
			: decorations.map(transaction.changes);
	},
	provide: (field) => EditorView.decorations.from(field),
});

function buildYamlValueDecorations(state: EditorView["state"]): DecorationSet {
	const builder = new RangeSetBuilder<Decoration>();

	for (let lineNumber = 1; lineNumber <= state.doc.lines; lineNumber += 1) {
		const line = state.doc.line(lineNumber);
		addIndentDecorations(builder, line.from, line.text);
		addLineValueDecorations(builder, line.from, line.text);
	}

	return builder.finish();
}

function addIndentDecorations(
	builder: RangeSetBuilder<Decoration>,
	lineFrom: number,
	text: string,
) {
	const indentEnd = text.search(/\S/);
	if (indentEnd <= 0) return;

	for (let start = 0, depth = 0; start < indentEnd; start += 2, depth += 1) {
		builder.add(
			lineFrom + start,
			lineFrom + Math.min(start + 2, indentEnd),
			Decoration.mark({
				class: `kubecove-yaml-indent kubecove-yaml-indent-${depth % yamlIndentDepthClassCount}`,
			}),
		);
	}
}

function addLineValueDecorations(
	builder: RangeSetBuilder<Decoration>,
	lineFrom: number,
	text: string,
) {
	const commentStart = findYamlCommentStart(text);
	const contentEnd = trimEndIndex(text, commentStart ?? text.length);
	const colonIndex = findMappingColon(text, contentEnd);

	if (colonIndex !== null) {
		const start = trimStartIndex(text, colonIndex + 1, contentEnd);
		addScalarDecoration(builder, lineFrom, text, start, contentEnd);
		return;
	}

	const listItemStart = findListScalarStart(text, contentEnd);
	if (listItemStart !== null) {
		addScalarDecoration(builder, lineFrom, text, listItemStart, contentEnd);
	}
}

function addScalarDecoration(
	builder: RangeSetBuilder<Decoration>,
	lineFrom: number,
	text: string,
	start: number,
	end: number,
) {
	const valueStart = trimStartIndex(text, start, end);
	const valueEnd = trimEndIndex(text, end);
	if (valueStart >= valueEnd) return;
	const value = text.slice(valueStart, valueEnd);
	if (value === "|" || value === ">" || value.startsWith("#")) return;
	builder.add(
		lineFrom + valueStart,
		lineFrom + valueEnd,
		Decoration.mark({ class: yamlScalarClass(value) }),
	);
}

function yamlScalarClass(value: string): string {
	const trimmed = value.trim();
	if (trimmed.startsWith("'") || trimmed.startsWith('"')) {
		return "kubecove-yaml-string";
	}
	if (/^(?:true|false|null|~)$/i.test(trimmed)) return "kubecove-yaml-bool";
	if (/^[+-]?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(trimmed)) {
		return "kubecove-yaml-number";
	}
	return "kubecove-yaml-string";
}

function findMappingColon(text: string, end: number): number | null {
	let quote: "'" | '"' | null = null;
	for (let index = 0; index < end; index += 1) {
		const char = text[index];
		if ((char === "'" || char === '"') && text[index - 1] !== "\\") {
			quote = quote === char ? null : quote ?? char;
			continue;
		}
		if (quote || char !== ":") continue;
		const next = text[index + 1];
		if (next === undefined || /\s/.test(next)) return index;
	}
	return null;
}

function findListScalarStart(text: string, end: number): number | null {
	const start = trimStartIndex(text, 0, end);
	if (text[start] !== "-" || !/\s/.test(text[start + 1] ?? "")) return null;
	const valueStart = trimStartIndex(text, start + 1, end);
	if (valueStart >= end) return null;
	const value = text.slice(valueStart, end);
	return findMappingColon(value, value.length) === null ? valueStart : null;
}

function findYamlCommentStart(text: string): number | null {
	let quote: "'" | '"' | null = null;
	for (let index = 0; index < text.length; index += 1) {
		const char = text[index];
		if ((char === "'" || char === '"') && text[index - 1] !== "\\") {
			quote = quote === char ? null : quote ?? char;
			continue;
		}
		if (!quote && char === "#" && /\s|^/.test(text[index - 1] ?? " ")) {
			return index;
		}
	}
	return null;
}

function trimStartIndex(text: string, start: number, end: number): number {
	let index = start;
	while (index < end && /\s/.test(text[index])) index += 1;
	return index;
}

function trimEndIndex(text: string, end: number): number {
	let index = end;
	while (index > 0 && /\s/.test(text[index - 1])) index -= 1;
	return index;
}

export function YamlCodeViewer({
	value,
	editable = false,
	onChange,
	minHeight = "360px",
	extraDiagnostics,
}: YamlCodeViewerProps) {
	const extensions = useMemo(
		() => [
			yaml(),
			EditorView.lineWrapping,
			yamlEditorTheme,
			syntaxHighlighting(yamlHighlightStyle),
			yamlValueDecorations,
			lintGutter(),
			linter(
				async (view) => {
					const document = view.state.doc.toString();
					const diagnostics = yamlDiagnostics(document);
					if (!extraDiagnostics) return diagnostics;
					return [...diagnostics, ...(await extraDiagnostics(document))];
				},
				{ delay: 450 },
			),
			keymap.of(lintKeymap),
		],
		[extraDiagnostics],
	);

	return (
		<div className="overflow-hidden rounded-md border bg-card shadow-sm">
			<CodeMirror
				value={value}
				extensions={extensions}
				editable={editable}
				basicSetup={{
					lineNumbers: true,
					foldGutter: true,
					highlightActiveLine: editable,
					highlightActiveLineGutter: editable,
				}}
				minHeight={minHeight}
				onChange={onChange}
			/>
		</div>
	);
}

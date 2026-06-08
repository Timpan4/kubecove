import type { Extension } from "@codemirror/state";
import { lintKeymap } from "@codemirror/lint";

const yamlIndentDepthClassCount = 6;

let yamlCodeViewerExtensionsPromise: Promise<readonly Extension[]> | null = null;

type EditorDoc = {
	lines: number;
	line: (lineNumber: number) => { from: number; text: string };
};

type StateLike = { doc: EditorDoc };

type CmStateLike = {
	StateField: {
		define(config: {
			create(state: StateLike): unknown;
			update(decorations: unknown, transaction: { docChanged: boolean; state?: StateLike }): unknown;
			provide: (field: unknown) => unknown;
		}): unknown;
	};
	RangeSetBuilder: new (...args: any[]) => {
		add(from: number, to: number, value: unknown): void;
		finish(): unknown;
		map(changes: unknown): unknown;
	};
};

type CmViewLike = {
	Decoration: {
		mark: (options: { class: string }) => unknown;
	};
	EditorView: {
		theme: (styles: Record<string, Record<string, unknown>>, options?: unknown) => unknown;
		decorations: { from: (field: unknown) => unknown };
		lineWrapping: unknown;
	};
	keymap: {
		of: (ext: unknown) => unknown;
	};
};

export async function loadYamlCodeViewerExtensions(): Promise<readonly Extension[]> {
	if (!yamlCodeViewerExtensionsPromise) {
		yamlCodeViewerExtensionsPromise = buildYamlCodeViewerExtensions();
	}
	return yamlCodeViewerExtensionsPromise;
}

async function buildYamlCodeViewerExtensions(): Promise<readonly Extension[]> {
	const cmState = (await import("@codemirror/state")) as unknown as CmStateLike;
	const cmView = (await import("@codemirror/view")) as unknown as CmViewLike;

	const yamlEditorTheme = cmView.EditorView.theme(editorThemeStyles, { dark: true });

	const yamlValueDecorations = cmState.StateField.define({
		create(state) {
			return buildYamlValueDecorations(state, cmState.RangeSetBuilder, cmView.Decoration);
		},
		update(decorations, transaction) {
			return transaction.docChanged && transaction.state
				? buildYamlValueDecorations(transaction.state, cmState.RangeSetBuilder, cmView.Decoration)
				: decorations;
		},
		provide: (field) => cmView.EditorView.decorations.from(field),
	});

	return [
		yamlEditorTheme,
		cmView.EditorView.lineWrapping,
		yamlValueDecorations,
		cmView.keymap.of(lintKeymap),
	] as unknown as readonly Extension[];
}

const editorThemeStyles: Record<string, Record<string, unknown>> = {
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
		backgroundColor: "color-mix(in oklch, var(--kubecove-yaml-indent-color) 11%, transparent)",
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
		backgroundColor: "color-mix(in oklch, var(--primary) 62%, transparent)",
	},
	".cm-selectionMatch": {
		backgroundColor: "color-mix(in oklch, var(--primary) 27%, transparent)",
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
};

function buildYamlValueDecorations(
	state: StateLike | null,
	RangeSetBuilderCtor: CmStateLike["RangeSetBuilder"],
	DecorationCtor: CmViewLike["Decoration"],
) {
	if (!state) return null;

	const builder = new RangeSetBuilderCtor();
	const viewState = state.doc;

	for (let lineNumber = 1; lineNumber <= viewState.lines; lineNumber += 1) {
		const line = viewState.line(lineNumber);
		addIndentDecorations(builder, DecorationCtor, line.from, line.text);
		addLineValueDecorations(builder, DecorationCtor, line.from, line.text);
	}

	return builder.finish();
}

function addIndentDecorations(
	builder: { add: (...args: any[]) => void },
	DecorationCtor: CmViewLike["Decoration"],
	lineFrom: number,
	text: string,
) {
	const indentEnd = text.search(/\S/);
	if (indentEnd <= 0) return;

	for (let start = 0, depth = 0; start < indentEnd; start += 2, depth += 1) {
		builder.add(
			lineFrom + start,
			lineFrom + Math.min(start + 2, indentEnd),
			DecorationCtor.mark({
				class: `kubecove-yaml-indent kubecove-yaml-indent-${depth % yamlIndentDepthClassCount}`,
			}),
		);
	}
}

function addLineValueDecorations(
	builder: { add: (...args: any[]) => void },
	DecorationCtor: CmViewLike["Decoration"],
	lineFrom: number,
	text: string,
) {
	const commentStart = findYamlCommentStart(text);
	const contentEnd = trimEndIndex(text, commentStart ?? text.length);
	const colonIndex = findMappingColon(text, contentEnd);

	if (colonIndex !== null) {
		const start = trimStartIndex(text, colonIndex + 1, contentEnd);
		addScalarDecoration(builder, DecorationCtor, lineFrom, text, start, contentEnd);
		return;
	}

	const listItemStart = findListScalarStart(text, contentEnd);
	if (listItemStart !== null) {
		addScalarDecoration(builder, DecorationCtor, lineFrom, text, listItemStart, contentEnd);
	}
}

function addScalarDecoration(
	builder: { add: (...args: any[]) => void },
	DecorationCtor: CmViewLike["Decoration"],
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
		DecorationCtor.mark({ class: yamlScalarClass(value) }),
	);
}

function yamlScalarClass(value: string): string {
	const trimmed = value.trim();
	if (trimmed.startsWith("'") || trimmed.startsWith('"')) return "kubecove-yaml-string";
	if (/^(?:true|false|null|~)$/i.test(trimmed)) return "kubecove-yaml-bool";
	if (/^[+-]?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(trimmed)) return "kubecove-yaml-number";
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

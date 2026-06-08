import CodeMirror from "@uiw/react-codemirror";
import { yaml } from "@codemirror/lang-yaml";
import type { Extension } from "@codemirror/state";
import {
	HighlightStyle,
	syntaxHighlighting,
} from "@codemirror/language";
import { linter, lintGutter, type Diagnostic } from "@codemirror/lint";
import { tags } from "@lezer/highlight";
import { useEffect, useMemo, useState } from "react";
import { type YAMLError } from "yaml";
import { parseYamlErrors } from "@/lib/yamlFormat";
import { loadYamlCodeViewerExtensions } from "@/components/yamlCodeViewerExtensions";

interface YamlCodeViewerProps {
	value: string;
	editable?: boolean;
	onChange?: (value: string) => void;
	minHeight?: string;
	extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
}

const yamlHighlightStyle = HighlightStyle.define([
	{ tag: tags.keyword, color: "oklch(0.78 0.12 250)" },
	{ tag: [tags.atom, tags.bool], color: "oklch(0.8 0.12 305)" },
	{ tag: tags.number, color: "oklch(0.82 0.12 78)" },
	{ tag: tags.string, color: "oklch(0.82 0.08 145)" },
	{ tag: tags.propertyName, color: "oklch(0.8 0.11 230)" },
	{ tag: [tags.definition(tags.propertyName)], color: "oklch(0.84 0.1 225)" },
	{ tag: tags.comment, color: "var(--muted-foreground)" },
	{ tag: tags.punctuation, color: "oklch(0.78 0.01 90)" },
	{ tag: tags.operator, color: "oklch(0.78 0.08 20)" },
	{ tag: tags.invalid, color: "var(--destructive)" },
]);

function yamlDiagnostics(value: string): Diagnostic[] {
	if (value.trim().length === 0) return [];
	try {
		const document = parseYamlErrors(value);
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

export function YamlCodeViewer({
	value,
	editable = false,
	onChange,
	minHeight = "360px",
	extraDiagnostics,
}: YamlCodeViewerProps) {
	const [editorExtensions, setEditorExtensions] = useState<readonly Extension[]>([]);

	useEffect(() => {
		let isCancelled = false;
		void loadYamlCodeViewerExtensions().then((extensions) => {
			if (!isCancelled) {
				setEditorExtensions(extensions);
			}
		});

		return () => {
			isCancelled = true;
		};
	}, []);

	const extensions = useMemo(
		() => [
			yaml(),
			syntaxHighlighting(yamlHighlightStyle),
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
			...editorExtensions,
		],
		[editorExtensions, extraDiagnostics],
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

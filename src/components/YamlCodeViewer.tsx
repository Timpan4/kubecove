import CodeMirror from "@uiw/react-codemirror";
import { yaml } from "@codemirror/lang-yaml";
import type { Extension } from "@codemirror/state";
import { syntaxHighlighting } from "@codemirror/language";
import { linter, lintGutter, type Diagnostic } from "@codemirror/lint";
import { useEffect, useMemo, useState } from "react";
import { loadYamlCodeViewerExtensions } from "@/components/yamlCodeViewerExtensions";
import { yamlDiagnostics, yamlHighlightStyle } from "@/components/yamlCodeViewerDiagnostics";

interface YamlCodeViewerProps {
	value: string;
	editable?: boolean;
	onChange?: (value: string) => void;
	minHeight?: string;
	extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
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

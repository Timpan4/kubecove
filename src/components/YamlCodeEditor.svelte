<script lang="ts">
	import { yaml } from "@codemirror/lang-yaml";
	import { foldGutter, syntaxHighlighting } from "@codemirror/language";
	import { lintGutter, linter, type Diagnostic } from "@codemirror/lint";
	import {
		EditorState,
		RangeSetBuilder,
		StateEffect,
		StateField,
		type Extension,
	} from "@codemirror/state";
	import {
		Decoration,
		type DecorationSet,
		EditorView,
		WidgetType,
		highlightActiveLine,
		highlightActiveLineGutter,
		lineNumbers,
	} from "@codemirror/view";
	import { onMount } from "svelte";
	import { loadYamlCodeViewerExtensions } from "@/components/yamlCodeViewerExtensions";
	import { yamlDiagnostics, yamlHighlightStyle } from "@/components/yamlCodeViewerDiagnostics";

	let {
		value = $bindable(""),
		editable = false,
		minHeight = "360px",
		extraDiagnostics,
		showErrorLens = true,
		onDiagnostics,
		onChange,
	}: {
		value: string;
		editable?: boolean;
		minHeight?: string;
		extraDiagnostics?: (value: string) => Promise<Diagnostic[]> | Diagnostic[];
		showErrorLens?: boolean;
		onDiagnostics?: (diagnostics: Diagnostic[]) => void;
		onChange?: (value: string) => void;
	} = $props();

	let host: HTMLDivElement | undefined = $state();
	let view: EditorView | null = $state(null);

	const setErrorLensDiagnostics = StateEffect.define<Diagnostic[]>();

	class ErrorLensWidget extends WidgetType {
		private diagnostic: Diagnostic;

		constructor(diagnostic: Diagnostic) {
			super();
			this.diagnostic = diagnostic;
		}

		toDOM() {
			const element = document.createElement("span");
			element.className = `cm-yaml-error-lens cm-yaml-error-lens-${this.diagnostic.severity}`;
			element.textContent = `${this.diagnostic.source ? `${this.diagnostic.source}: ` : ""}${this.diagnostic.message}`;
			return element;
		}
	}

	const errorLensField = StateField.define<DecorationSet>({
		create: () => Decoration.none,
		update(decorations, transaction) {
			for (const effect of transaction.effects) {
				if (!effect.is(setErrorLensDiagnostics)) continue;
				const builder = new RangeSetBuilder<Decoration>();
				const seenLines = new Set<number>();
				for (const diagnostic of effect.value) {
					if (diagnostic.severity === "info") continue;
					const line = transaction.state.doc.lineAt(diagnostic.from);
					if (seenLines.has(line.number)) continue;
					seenLines.add(line.number);
					builder.add(
						line.to,
						line.to,
						Decoration.widget({
							widget: new ErrorLensWidget(diagnostic),
							side: 1,
						}),
					);
				}
				return builder.finish();
			}
			return decorations.map(transaction.changes);
		},
		provide: (field) => EditorView.decorations.from(field),
	});

	function extensions(extraExtensions: readonly Extension[]): Extension[] {
		return [
			lineNumbers(),
			foldGutter(),
			EditorView.editable.of(editable),
			EditorState.readOnly.of(!editable),
			editable ? highlightActiveLine() : [],
			editable ? highlightActiveLineGutter() : [],
			yaml(),
			syntaxHighlighting(yamlHighlightStyle),
			lintGutter(),
			linter(
				async (editorView) => {
					const document = editorView.state.doc.toString();
					const diagnostics = [
						...yamlDiagnostics(document),
						...(extraDiagnostics ? await extraDiagnostics(document) : []),
					];
					onDiagnostics?.(diagnostics);
					editorView.dispatch({
						effects: setErrorLensDiagnostics.of(showErrorLens ? diagnostics : []),
					});
					return diagnostics;
				},
				{ delay: 450 },
			),
			errorLensField,
			EditorView.updateListener.of((update) => {
				if (!update.docChanged) return;
				const nextValue = update.state.doc.toString();
				value = nextValue;
				onChange?.(nextValue);
			}),
			...extraExtensions,
		];
	}

	onMount(() => {
		let cancelled = false;
		void loadYamlCodeViewerExtensions().then((extraExtensions) => {
			if (cancelled || !host) return;
			view = new EditorView({
				state: EditorState.create({
					doc: value,
					extensions: extensions(extraExtensions),
				}),
				parent: host,
			});
		});

		return () => {
			cancelled = true;
			view?.destroy();
			view = null;
		};
	});

	$effect(() => {
		if (!view) return;
		const currentValue = view.state.doc.toString();
		if (value === currentValue) return;
		view.dispatch({
			changes: { from: 0, to: currentValue.length, insert: value },
		});
	});
</script>

<div
	class="yaml-code-editor overflow-hidden rounded-md border bg-card shadow-sm"
	style={`--yaml-code-editor-min-height: ${minHeight}`}
	bind:this={host}
></div>

<style>
	.yaml-code-editor :global(.cm-editor) {
		min-height: var(--yaml-code-editor-min-height);
	}

	.yaml-code-editor :global(.cm-scroller) {
		min-height: var(--yaml-code-editor-min-height);
	}

	.yaml-code-editor :global(.cm-yaml-error-lens) {
		display: block;
		max-width: min(72ch, calc(100% - 1rem));
		margin: 0.25rem 0 0.1rem 1rem;
		white-space: normal;
		font-size: 0.75rem;
		line-height: 1.35;
		opacity: 0.9;
	}

	.yaml-code-editor :global(.cm-yaml-error-lens-error) {
		color: hsl(var(--destructive));
	}

	.yaml-code-editor :global(.cm-yaml-error-lens-warning) {
		color: hsl(var(--warning, var(--muted-foreground)));
	}
</style>

import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import { parse } from "yaml";
import type { Step } from "./contract";
import type { YamlEditorDiagnostic } from "./contract-steps";

export async function yamlEditorAction(page: Page, step: Step) {
	switch (step.action) {
		case "expectYamlAbsent": {
			const editor = page.locator(".cm-content");
			await editor.waitFor();
			for (const fragment of step.fragments) if ((await editor.textContent())?.includes(fragment)) throw new Error(`Unexpected YAML field ${fragment}`);
			break;
		}
		case "clickActionEntry": await page.getByRole("textbox", { name: step.label, exact: true }).locator("xpath=..").getByRole("button", { name: step.button, exact: true }).click(); break;
		case "fillContainerImage": await page.getByRole("heading", { name: step.container, exact: true }).locator("xpath=..").getByRole("textbox", { name: "Container image", exact: true }).fill(step.value); break;
		case "openActionEditor": await page.locator(`[data-resource-action="${step.row}"]`).getByRole("button", { name: step.row === "rollback" ? "Roll back" : "Edit", exact: true }).click(); break;
		case "fillActionField": await page.getByRole("textbox", { name: step.label, exact: true }).fill(step.value); break;
		case "chooseActionRevision": await page.locator(`input[name="resource-action-revision"][value="${step.revision}"]`).check(); break;
		case "selectYamlShape": case "selectYamlEncoding":
			await page.getByRole("combobox", { name: step.action === "selectYamlShape" ? "YAML shape" : "YAML encoding", exact: true }).click();
			await page.getByRole("option", { name: step.value, exact: true }).click();
			break;
		case "expectYamlPicker": await page.getByRole("combobox", { name: step.label, exact: true }).filter({ hasText: step.value }).waitFor(); break;
		case "replaceYamlDraft": await page.locator(".cm-content[contenteditable=true]").fill(step.value); break;
		case "formatYaml": await page.getByRole("button", { name: /^Format \(/ }).click(); break;
		case "expectYamlDocument": {
			// biome-ignore lint/performance/useDomNodeTextContent: CodeMirror renders document lines as blocks, so rendered text preserves the YAML line breaks.
			const text = await page.locator(".cm-content").innerText();
			if (!isDeepStrictEqual(parse(text), JSON.parse(step.value))) throw new Error("Formatting changed YAML content");
			break;
		}
		case "foldYamlLine": {
			// The fold gutter marks a line at the same height as its line number.
			const number = page.locator(".cm-lineNumbers .cm-gutterElement").filter({ visible: true }).getByText(String(step.line), { exact: true });
			// The editor mounts after its extensions load, which can follow the tab switch.
			await number.waitFor();
			const box = await number.boundingBox();
			if (!box) throw new Error(`YAML line ${step.line} is not shown`);
			const markers = page.locator(".cm-foldGutter .cm-gutterElement").filter({ visible: true });
			for (const marker of await markers.all()) {
				const markerBox = await marker.boundingBox();
				if (markerBox && Math.abs(markerBox.y - box.y) < 1) {
					await marker.locator("span").click();
					return;
				}
			}
			throw new Error(`YAML line ${step.line} has no fold marker`);
		}
		case "expectYamlVisibleLines": {
			// The editor mounts after its extensions load, and a fold redraws the gutter, so wait for the lines.
			// Visible as Playwright counts it: a nonempty box without visibility: hidden.
			await page.waitForFunction((want) => {
				const numbers: number[] = [];
				for (const element of document.querySelectorAll(".cm-lineNumbers .cm-gutterElement")) {
					const box = element.getBoundingClientRect();
					if (box.width > 0 && box.height > 0 && element.checkVisibility({ visibilityProperty: true })) numbers.push(Number(element.textContent));
				}
				return JSON.stringify(numbers) === want;
			}, JSON.stringify(step.lines)).catch(async () => {
				const visible = (await page.locator(".cm-lineNumbers .cm-gutterElement").filter({ visible: true }).allTextContents()).map(Number);
				throw new Error(`Visible YAML lines ${visible.join(",")}, expected ${step.lines.join(",")}`);
			});
			break;
		}
		case "expectYamlEditorDiagnostics": await expectEditorDiagnostics(page, step.diagnostics); break;
		default:
			throw new Error(`unhandled YAML editor action: ${step.action}`);
	}
}

/** Waits until the editor marks exactly the expected diagnostics, which the linter sets after its delay. */
async function expectEditorDiagnostics(page: Page, expected: YamlEditorDiagnostic[]): Promise<void> {
	try {
		await page.waitForFunction(editorDiagnostics, expected);
	} catch (error) {
		const actual = await page.evaluate(editorDiagnostics, null);
		throw new Error(`Editor diagnostics ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`, { cause: error });
	}
}

/**
 * Runs in the page. Reads every diagnostic in the YAML editor's lint state, sorted by position.
 * Without `expected`, returns them. With it, returns them only when they match, and false otherwise,
 * so `waitForFunction` polls until they do. The editor view is the one `EditorView.findFromDOM` finds
 * through the content element, and the lint state is the state field holding `diagnostics`, `panel` and `selected`.
 */
function editorDiagnostics(expected: YamlEditorDiagnostic[] | null) {
	type LintDiagnostic = { from: number; to: number; severity: string; source?: string; message: string };
	type LintState = { diagnostics: { between(from: number, to: number, visit: (from: number, to: number, value: { spec: { diagnostics?: LintDiagnostic[] } }) => void): void } };
	type Doc = { length: number; lineAt(offset: number): { number: number; from: number } };
	const isObjectValue = <Value>(value: Value): value is Value & object => Object(value) === value;
	const field = <Target>(target: Target, key: string) => (isObjectValue(target) ? Object(target)[key] : undefined);
	const isLintState = <Value>(value: Value): value is Value & LintState => isObjectValue(value) && "diagnostics" in value && "panel" in value && "selected" in value;
	const isDoc = <Value>(value: Value): value is Value & Doc => isObjectValue(value) && "lineAt" in value && "length" in value;
	const content = document.querySelector(".cm-content[contenteditable=true]");
	const state = field(field(field(field(content, "cmTile"), "root"), "view"), "state");
	const doc = field(state, "doc");
	const values = field(state, "values");
	const lint = Array.isArray(values) ? values.find((value): value is LintState => isLintState(value)) : undefined;
	if (!isDoc(doc) || !lint) return expected ? false : null;
	const found = new Set<LintDiagnostic>();
	lint.diagnostics.between(0, doc.length, (_from, _to, value) => {
		for (const diagnostic of value.spec.diagnostics ?? []) found.add(diagnostic);
	});
	const position = (offset: number): [number, number] => {
		const line = doc.lineAt(offset);
		return [line.number, offset - line.from + 1];
	};
	const actual = [...found]
		.sort((a, b) => a.from - b.from || a.to - b.to)
		.map((diagnostic) => ({ from: position(diagnostic.from), to: position(diagnostic.to), severity: diagnostic.severity, source: diagnostic.source ?? null, message: diagnostic.message }));
	if (!expected) return actual;
	const same = actual.length === expected.length && expected.every((want, index) => {
		const got = actual[index];
		return got !== undefined && got.from.join() === want.from.join() && got.to.join() === want.to.join() && got.severity === want.severity && got.source === want.source && (want.message === undefined || got.message === want.message);
	});
	return same ? actual : false;
}

import { sharedActions, type WebActions } from "./web-action";
import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import { parse } from "yaml";
import type { YamlEditorDiagnostic } from "./contract-yaml";

export const yamlEditorActions = {
	async expectYamlAbsent(page, step) {
		const document = await yamlDocument(page);
		for (const fragment of step.fragments) if (document.includes(fragment)) throw new Error(`Unexpected YAML field ${fragment}`);
	},
	async clickActionEntry(page, step) {
		await page.getByRole("textbox", { name: step.label, exact: true }).locator("xpath=..").getByRole("button", { name: step.button, exact: true }).click();
	},
	async fillContainerImage(page, step) {
		await page.getByRole("heading", { name: step.container, exact: true }).locator("xpath=..").getByRole("textbox", { name: "Container image", exact: true }).fill(step.value);
	},
	async openActionEditor(page, step) {
		await page.locator(`[data-resource-action="${step.row}"]`).getByRole("button", { name: step.row === "rollback" ? "Roll back" : "Edit", exact: true }).click();
	},
	async fillActionField(page, step) {
		await page.getByRole("textbox", { name: step.label, exact: true }).fill(step.value);
	},
	async chooseActionRevision(page, step) {
		await page.locator(`input[name="resource-action-revision"][value="${step.revision}"]`).check();
	},
	...sharedActions(["selectYamlShape", "selectYamlEncoding"], async (page, step) => {
		await page.getByRole("combobox", { name: step.action === "selectYamlShape" ? "YAML shape" : "YAML encoding", exact: true }).click();
		await page.getByRole("option", { name: step.value, exact: true }).click();
	}),
	async expectYamlPicker(page, step) {
		await page.getByRole("combobox", { name: step.label, exact: true }).filter({ hasText: step.value }).waitFor();
	},
	async replaceYamlDraft(page, step) {
		await page.locator(".cm-content[contenteditable=true]").fill(step.value);
	},
	async formatYaml(page) {
		await page.getByRole("button", { name: /^Format \(/ }).click();
	},
	async expectYamlDocument(page, step) {
		if (!isDeepStrictEqual(parse(await yamlDocument(page)), JSON.parse(step.value))) throw new Error("Formatting changed YAML content");
	},
	async expectYamlEditorFillsPanel(page, step) {
		const tab = page.getByRole("tabpanel", { name: "YAML", exact: true });
		// The editor mounts after its extensions load, so wait for it before taking the tab.
		await tab.locator(".cm-editor").waitFor();
		const panel = await tab.elementHandle();
		if (!panel) throw new Error("Missing YAML tab");
		const previous = measuredYamlEditors.get(page) ?? null;
		if (step.grows && !previous) throw new Error("An earlier expectYamlEditorFillsPanel must record the heights to grow from");
		// One wait over the geometry, the inline marker and, for `grows`, the growth, so a pre-resize
		// frame cannot end it early.
		const handle = await page.waitForFunction(yamlEditorReady, { panel, previous: step.grows ? previous : null }).catch(async (error) => {
			// Best effort: the page or context may be gone, and that must not hide the wait failure.
			const last = await panel.evaluate((element) => ("lastMeasurement" in element ? element.lastMeasurement : null)).catch(() => null);
			throw new Error(`The YAML editor did not fill its tab with an inline whitespace marker${step.grows ? " and grow" : ""}: from ${JSON.stringify(step.grows ? previous : null)}, last ${JSON.stringify(last)}`, { cause: error });
		});
		const measured = await handle.jsonValue();
		// waitForFunction resolves only on a truthy predicate result, so this is a type guard.
		if (!measured) throw new Error("The YAML editor wait resolved without a measurement");
		measuredYamlEditors.set(page, measured);
	},
	async foldYamlLine(page, step) {
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
	},
	async expectYamlVisibleLines(page, step) {
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
	},
	async expectYamlEditorDiagnostics(page, step) {
		await expectEditorDiagnostics(page, step.diagnostics);
	},
} satisfies Partial<WebActions>;

/** Bounds in CSS pixels, read by `yamlEditorReady`. The bottoms are page positions. */
type YamlMeasurement = { panelHeight: number; panelBottom: number; viewportHeight: number; viewportBottom: number; editorBottom: number; inlineMarker: string };

/** The YAML tab's panel and viewport heights from the page's last `expectYamlEditorFillsPanel`. */
type YamlHeights = Pick<YamlMeasurement, "panelHeight" | "viewportHeight">;

/** The YAML tab's heights from the page's last `expectYamlEditorFillsPanel`, which a `grows` check compares against. */
const measuredYamlEditors = new WeakMap<Page, YamlHeights>();

/**
 * Runs in the page. Returns the measurement once the editor fills the tab, an inline whitespace
 * marker is painted and, when `previous` is set, both heights exceed it. Returns false otherwise,
 * so `waitForFunction` polls until all hold.
 */
function yamlEditorReady(arg: { panel: Element | null; previous: YamlHeights | null }): YamlMeasurement | false {
	const { panel, previous } = arg;
	const element = panel?.querySelector<HTMLElement>(".cm-editor");
	const scroller = element?.querySelector<HTMLElement>(".cm-scroller");
	if (!panel || !element || !scroller) return false;
	const panelBox = panel.getBoundingClientRect();
	const editorBox = element.getBoundingClientRect();
	const scrollerBox = scroller.getBoundingClientRect();
	const inlineSpace = [...element.querySelectorAll<HTMLElement>(".cm-highlightSpace")].find((space) => !space.closest(".kubecove-yaml-indent"));
	const inlineMarker = inlineSpace ? getComputedStyle(inlineSpace).backgroundImage : "none";
	const measurement = {
		panelHeight: panelBox.height,
		panelBottom: panelBox.bottom,
		viewportHeight: scrollerBox.height,
		viewportBottom: scrollerBox.bottom,
		editorBottom: editorBox.bottom,
		inlineMarker,
	};
	// Kept on the panel so a failed wait can report the last frame it saw.
	Object.assign(panel, { lastMeasurement: measurement });
	// Match the editor's 1px frame plus browser rounding.
	const fills = Math.abs(editorBox.bottom - panelBox.bottom) <= 1 && Math.abs(editorBox.bottom - scrollerBox.bottom) <= 1;
	const grew = previous === null || (measurement.panelHeight > previous.panelHeight && measurement.viewportHeight > previous.viewportHeight);
	return fills && inlineMarker !== "none" && grew ? measurement : false;
}

/**
 * The whole document of the page's one YAML editor, from the accessor `YamlCodeEditor.svelte`
 * adds in development builds. The editor mounts after its extensions load, so this waits for it.
 */
async function yamlDocument(page: Page): Promise<string> {
	const handle = await page.waitForFunction(() => {
		const hosts = [...document.querySelectorAll<HTMLElement & { yamlDocument?: () => string }>(".yaml-code-editor")];
		const mounted = hosts.flatMap((host) => (host.yamlDocument ? [host.yamlDocument()] : []));
		// An object, so an empty document still ends the wait.
		return mounted.length ? { editors: hosts.length, text: mounted[0] ?? "" } : null;
	});
	const value = await handle.jsonValue();
	// Like a strict locator, refuse to pick one editor of several.
	if (value?.editors !== 1) throw new Error(`Expected one YAML editor, found ${value?.editors}`);
	return value.text;
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

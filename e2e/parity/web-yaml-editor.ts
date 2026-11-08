import { isDeepStrictEqual } from "node:util";
import type { Page } from "playwright-core";
import { parse } from "yaml";
import type { Step } from "./contract";

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
			const numbers = await page.locator(".cm-lineNumbers .cm-gutterElement").filter({ visible: true }).allTextContents();
			const visible = numbers.map(Number);
			if (!isDeepStrictEqual(visible, step.lines)) throw new Error(`Visible YAML lines ${visible.join(",")}, expected ${step.lines.join(",")}`);
			break;
		}
		default:
			throw new Error(`unhandled YAML editor action: ${step.action}`);
	}
}

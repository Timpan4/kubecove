import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..", "src", "components", "ui", "svelte");

function readPrimitive(file: string) {
	return readFileSync(join(root, file), "utf8");
}

describe("svelte ui primitives", () => {
	test("maps semantic z-index utilities to runtime stacking tokens", () => {
		const appCss = readFileSync(join(import.meta.dir, "..", "src", "App.css"), "utf8");
		const theme = appCss.match(/@theme inline \{([\s\S]*?)\n\}/)?.[1] ?? "";

		for (const token of ["base", "content", "sticky", "overlay", "popover", "toast", "dialog"]) {
			expect(theme).toContain(`--z-index-${token}: var(--z-${token});`);
		}
		expect(readPrimitive("PopoverContent.svelte")).toContain("z-popover");
		expect(readPrimitive("TooltipContent.svelte")).toContain("z-popover");
		expect(readPrimitive("SelectContent.svelte")).toContain("z-popover");
		expect(readPrimitive("SheetContent.svelte")).toContain("z-dialog");
		expect(readPrimitive("SheetContent.svelte")).not.toContain("z-50");
	});

	test("exports launcher and mirror primitives", () => {
		const index = readPrimitive("index.ts");
		for (const name of [
			"Alert",
			"Badge",
			"Button",
			"Card",
			"Checkbox",
			"Dialog",
			"Empty",
			"Field",
			"Input",
			"InputGroup",
			"Popover",
			"ResizablePanelGroup",
			"Select",
			"Separator",
			"Sheet",
			"Sidebar",
			"Skeleton",
			"Spinner",
			"Table",
			"Tabs",
			"Textarea",
			"Tooltip",
		]) {
			expect(index).toContain(`export { default as ${name} }`);
		}
	});

	test("keeps button variant and size names", () => {
		const classes = readPrimitive("classes.ts");

		for (const token of [
			"default",
			"outline",
			"secondary",
			"ghost",
			"destructive",
			"link",
			"icon-xs",
			"icon-sm",
			"icon-lg",
		]) {
			expect(classes).toContain(token);
		}
	});

	test("lets Bits render selected select value when no custom children exist", () => {
		const selectValue = readPrimitive("SelectValue.svelte");

		expect(selectValue).toContain("{#if children}");
		expect(selectValue).toContain("<SelectPrimitive.Value data-slot=\"select-value\" {...rest} />");
	});

	test("reserves room for select item checkmark", () => {
		const selectItem = readPrimitive("SelectItem.svelte");

		expect(selectItem).toContain("pr-7");
		expect(selectItem).toContain("*:[span]:last:truncate");
	});
});

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

describe("pod exec UI", () => {
	test("Svelte detail exposes live-session tabs only for guarded targets", () => {
		const source = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);
		const yamlPaneSource = readFileSync(
			"src/features/resource-detail/ResourceYamlPane.svelte",
			"utf8",
		);
		const readSpecSource = readFileSync(
			"src/features/resource-detail/resourceDetailReadSpec.ts",
			"utf8",
		);

		expect(source).toContain('const canShowExec = $derived(isPod)');
		expect(source).toContain('const canShowPortForward = $derived(');
		expect(source).toContain('{#if canShowExec}<TabsTrigger value="exec">Exec</TabsTrigger>{/if}');
		expect(source).toContain(
			'{#if canShowPortForward}<TabsTrigger value="portForward">Forward</TabsTrigger>{/if}',
		);
		expect(source).toContain("if (!isDetailTabAvailable(activeTab))");
		expect(source).toContain("function isDetailTabAvailable(tab: DetailTab): boolean");
		const resourceKeySnippet = [
			"`",
			"$",
			'{resource.cluster}:',
			"$",
			'{resource.apiVersion ?? ""}:',
			"$",
			"{resource.kind}:",
			"$",
			'{resource.namespace ?? ""}:',
			"$",
			"{resource.name}",
			"`",
		].join("");
		expect(readSpecSource).toContain(resourceKeySnippet);
		expect(source).toContain('diagnosticLog("detail.mount", { key })');
		expect(source).toContain('diagnosticLog("detail.unmount", { key })');
		expect(source).toContain('diagnosticLog("detail.tab.click", { key: resourceKey, tab: nextTab })');
		expect(source).toContain("withForegroundLoad(loadLabel, task)");
		expect(source).toContain('runDetailFetch("details", "resource-details"');
		expect(yamlPaneSource).toContain('runYamlFetch("resource-yaml"');
		expect(source).toContain('runDetailFetch("events", "resource-events"');
		expect(source).toContain("diagnosticResultSummary(result)");
		expect(yamlPaneSource).toContain("diagnosticResultSummary(result)");
	});

	test("Svelte exec keeps guarded target and kubeconfig source visible", () => {
		const source = [
			readFileSync("src/features/resource-detail/ExecTab.svelte", "utf8"),
			readFileSync("src/features/resource-detail/PodExecLaunchForm.svelte", "utf8"),
			readFileSync("src/features/resource-detail/PodExecSessionList.svelte", "utf8"),
		].join("\n");

		expect(source).toContain("$settingsStore.showKubeconfigSourceLabels");
		expect(source).toContain("podExecTarget(resource, selectedContainer)");
		expect(source).toContain("Kubeconfig source");
		expect(source).toContain("showKubeconfigSourceLabels && kubeconfigSourceKey");
		expect(source).toContain("showKubeconfigSourceLabels && item.kubeconfigSourceLabel");
	});

	test("Svelte exec shares selected container through bindable detail state", () => {
		const tabSource = readFileSync(
			"src/features/resource-detail/ExecTab.svelte",
			"utf8",
		);
		const detailSource = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);

		expect(tabSource).toContain('selectedContainer = $bindable("")');
		expect(detailSource).toContain("bind:selectedContainer");
		expect(tabSource).not.toContain("onSelectedContainerChange");
	});

	test("Svelte exec uses the real xterm live-session path", () => {
		const source = [
			readFileSync("src/features/resource-detail/ExecTab.svelte", "utf8"),
			readFileSync("src/features/resource-detail/execTerminal.ts", "utf8"),
		].join("\n");

		expect(source).toContain('@xterm/xterm/css/xterm.css');
		expect(source).toContain('import { FitAddon } from "@xterm/addon-fit"');
		expect(source).toContain('import { Terminal } from "@xterm/xterm"');
		expect(source).toContain("resizePodExecTerminal");
		expect(source).toContain("terminal.onData");
		expect(source).toContain("terminal.onResize");
		expect(source).toContain("bind:this={terminalHost}");
		expect(source).toContain("terminal?.cols ?? DEFAULT_COLS");
		expect(source).toContain("terminal?.rows ?? DEFAULT_ROWS");
		expect(source).not.toContain("Terminal output appears here.");
	});
});

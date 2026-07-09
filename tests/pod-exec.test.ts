import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import type { PodExecSessionSummary, ResourceSummary } from "../src/lib/types";
import {
	buildPodExecRequest,
	commandForPreset,
	isPodExecForResource,
	podExecCommandText,
	podExecTarget,
	sortPodExecSessions,
} from "../src/features/resource-detail/pod-exec-helpers";

const pod: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	namespace: "payments",
	name: "api-0",
	age: "1m",
};

const session: PodExecSessionSummary = {
	id: "pod-exec-1",
	clusterContext: "kind-dev",
	namespace: "payments",
	podName: "api-0",
	container: "api",
	command: ["/bin/sh"],
	stdin: true,
	tty: true,
	terminalCols: 100,
	terminalRows: 32,
	status: "running",
	startedAt: "2026-06-01T10:00:00Z",
};

describe("pod exec helpers", () => {
	test("builds exact shell preset requests with confirmation metadata", () => {
		const request = buildPodExecRequest(pod, {
			preset: "sh",
			customArgv: "",
			container: "api",
			cols: 100,
			rows: 32,
			confirmed: true,
		});

		expect(request).toEqual({
			clusterContext: "kind-dev",
			namespace: "payments",
			podName: "api-0",
			container: "api",
			command: ["/bin/sh"],
			stdin: true,
			tty: true,
			terminalSize: { cols: 100, rows: 32 },
			confirmation: {
				acknowledged: true,
				target: "kind-dev/payments/Pod/api-0/container/api",
				command: "[\"/bin/sh\"]",
			},
		});
	});

	test("requires Pod scope, namespace, command, terminal size, and confirmation", () => {
		expect(
			buildPodExecRequest(
				{ ...pod, kind: "Deployment", name: "api" },
				{
					preset: "sh",
					customArgv: "",
					cols: 100,
					rows: 32,
					confirmed: true,
				},
			),
		).toBe("Pod exec starts from an exact Pod");
		expect(
			buildPodExecRequest(
				{ ...pod, namespace: null },
				{
					preset: "sh",
					customArgv: "",
					cols: 100,
					rows: 32,
					confirmed: true,
				},
			),
		).toBe("Pod exec requires a namespace");
		expect(commandForPreset("custom", "\n ")).toBe("Custom argv is required");
		expect(
			buildPodExecRequest(pod, {
				preset: "sh",
				customArgv: "",
				cols: 0,
				rows: 32,
				confirmed: true,
			}),
		).toBe("Terminal size must be between 1 and 500 columns and rows");
		expect(
			buildPodExecRequest(pod, {
				preset: "sh",
				customArgv: "",
				cols: 100,
				rows: 32,
				confirmed: false,
			}),
		).toBe("Confirm the exact target and command before starting exec");
	});

	test("uses line-based custom argv instead of shell parsing", () => {
		expect(commandForPreset("custom", "/usr/bin/env\nprintenv")).toEqual([
			"/usr/bin/env",
			"printenv",
		]);
		expect(commandForPreset("bash", "")).toEqual(["/bin/bash"]);
		expect(podExecTarget(pod, "api")).toBe(
			"kind-dev/payments/Pod/api-0/container/api",
		);
		expect(podExecTarget(pod)).toBe(
			"kind-dev/payments/Pod/api-0/container/<default>",
		);
		expect(podExecCommandText(["/bin/sh", "-lc", "date"])).toBe(
			"[\"/bin/sh\",\"-lc\",\"date\"]",
		);
	});

	test("matches and sorts active sessions for a selected Pod", () => {
		expect(isPodExecForResource(session, pod)).toBe(true);
		expect(
			isPodExecForResource({ ...session, podName: "worker-0" }, pod),
		).toBe(false);
		expect(
			sortPodExecSessions([
				{ ...session, id: "b", startedAt: "2026-06-01T10:01:00Z" },
				{ ...session, id: "a", startedAt: "2026-06-01T10:00:00Z" },
			]).map((item) => item.id),
		).toEqual(["a", "b"]);
	});

	test("Svelte detail exposes live-session tabs only for guarded targets", () => {
		const source = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);
		const yamlPaneSource = readFileSync(
			"src/features/resource-detail/ResourceYamlPane.svelte",
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
		expect(source).toContain(resourceKeySnippet);
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
		const source = readFileSync(
			"src/features/resource-detail/ExecTab.svelte",
			"utf8",
		);

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
		const source = readFileSync(
			"src/features/resource-detail/ExecTab.svelte",
			"utf8",
		);

		expect(source).toContain('@xterm/xterm/css/xterm.css');
		expect(source).toContain('import { FitAddon } from "@xterm/addon-fit"');
		expect(source).toContain('import { Terminal as XtermTerminal } from "@xterm/xterm"');
		expect(source).toContain("resizePodExecTerminal");
		expect(source).toContain("nextTerminal.onData");
		expect(source).toContain("nextTerminal.onResize");
		expect(source).toContain("bind:this={terminalHost}");
		expect(source).toContain("terminal?.cols ?? DEFAULT_COLS");
		expect(source).toContain("terminal?.rows ?? DEFAULT_ROWS");
		expect(source).not.toContain("Terminal output appears here.");
	});
});

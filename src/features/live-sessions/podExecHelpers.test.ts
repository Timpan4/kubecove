import type { PodExecSessionSummary, ResourceSummary } from "@/lib/types";
import {
	buildPodExecRequest,
	commandForPreset,
	isPodExecForResource,
	podExecCommandText,
	podExecTarget,
} from "@/features/live-sessions";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

const pod: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	namespace: "payments",
	name: "api-0",
	age: "1m",
	health: "healthy",
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
				command: '["/bin/sh"]',
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
			'["/bin/sh","-lc","date"]',
		);
	});

	test("matches active sessions for a selected Pod", () => {
		expect(isPodExecForResource(session, pod)).toBe(true);
		expect(
			isPodExecForResource({ ...session, podName: "worker-0" }, pod),
		).toBe(false);
	});
});

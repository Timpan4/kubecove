import type { ResourceSummary } from "@/lib/types";
import {
	buildPodExecRequest,
	parsePodExecArgv,
	podExecCommandText,
	podExecTarget,
} from "@/features/live-sessions";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toEqual<Expected>(expected: Expected): void;
};

const pod: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	namespace: "payments",
	name: "api-0",
	age: "1m",
	health: "healthy",
};

describe("pod exec helpers", () => {
	test("builds exact shell requests with confirmation metadata", () => {
		const request = buildPodExecRequest(pod, {
			command: ["/bin/sh"],
			container: "api",
			cols: 100,
			rows: 32,
		});

		expect(request).toEqual({
			ok: true,
			request: {
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
			},
		});
	});

	test("requires Pod scope, namespace, command, and terminal size", () => {
		expect(
			buildPodExecRequest(
				{ ...pod, kind: "Deployment", name: "api" },
				{ command: ["/bin/sh"], cols: 100, rows: 32 },
			),
		).toEqual({ ok: false, message: "Pod exec starts from an exact Pod" });
		expect(
			buildPodExecRequest(
				{ ...pod, namespace: null },
				{ command: ["/bin/sh"], cols: 100, rows: 32 },
			),
		).toEqual({ ok: false, message: "Pod exec requires a namespace" });
		expect(buildPodExecRequest(pod, { command: parsePodExecArgv("\n "), cols: 100, rows: 32 })).toEqual({
			ok: false,
			message: "Enter a command to run",
		});
		expect(buildPodExecRequest(pod, { command: ["/bin/sh"], cols: 0, rows: 32 })).toEqual({
			ok: false,
			message: "Terminal size must be between 1 and 500 columns and rows",
		});
	});

	test("uses line-based custom argv instead of shell parsing", () => {
		expect(parsePodExecArgv("/usr/bin/env\nprintenv")).toEqual([
			"/usr/bin/env",
			"printenv",
		]);
		expect(parsePodExecArgv("sh -c 'echo $HOME'")).toEqual(["sh -c 'echo $HOME'"]);
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
});

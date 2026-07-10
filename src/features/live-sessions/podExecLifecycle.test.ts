import { createMockChannel, createMockTauriClient } from "@/lib/tauri";
import type { PodExecSessionMessage, PodExecSessionSummary } from "@/lib/types";
import { createWorkspaceRecord } from "@/lib/workspace-model";
import {
	podExecQueryOptions,
	podExecSessionsForWorkspace,
	startPodExec,
	stopPodExec,
} from "./podExecLifecycle";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function session(
	id: string,
	clusterContext: string,
	startedAt: string,
	kubeconfigSourceKey = "kubeconfigSource=default",
): PodExecSessionSummary {
	return {
		id,
		clusterContext,
		kubeconfigSourceKey,
		namespace: "payments",
		podName: "api-0",
		command: ["/bin/sh"],
		stdin: true,
		tty: true,
		terminalCols: 100,
		terminalRows: 32,
		status: "running",
		startedAt,
	};
}

describe("pod exec lifecycle", () => {
	test("owns sorted polling query options", async () => {
		const client = createMockTauriClient({
			list_pod_exec_sessions: [
				session("second", "kind-dev", "2026-07-10T00:01:00Z"),
				session("first", "kind-dev", "2026-07-10T00:00:00Z"),
			],
		});
		const options = podExecQueryOptions(client, {
			enabled: true,
			refetchInterval: 2_500,
		});

		expect(options.queryKey).toEqual(["pod-exec-sessions"]);
		expect(options.enabled).toBe(true);
		expect(options.refetchInterval).toBe(2_500);
		expect((await options.queryFn()).map((item) => item.id)).toEqual([
			"first",
			"second",
		]);
	});

	test("filters sessions by workspace contexts and kubeconfig source", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			clusterContexts: ["kind-dev", "prod"],
			namespaces: [],
		});
		const sessions = [
			session("dev", "kind-dev", "2026-07-10T00:00:00Z"),
			session("prod", "prod", "2026-07-10T00:01:00Z"),
			session("other-source", "kind-dev", "2026-07-10T00:02:00Z", "kubeconfigSource=other"),
			session("other-context", "staging", "2026-07-10T00:03:00Z"),
		];

		expect(
			podExecSessionsForWorkspace(
				sessions,
				workspace,
				"kubeconfigSource=default",
			).map((item) => item.id),
		).toEqual(["dev", "prod"]);
	});

	test("invalidates after start and stop", async () => {
		const calls: string[] = [];
		const started = session("exec-1", "kind-dev", "2026-07-10T00:00:00Z");
		const client = createMockTauriClient({
			start_pod_exec_session: () => {
				calls.push("start");
				return started;
			},
			stop_pod_exec_session: () => {
				calls.push("stop");
				return true;
			},
		});
		const invalidated: (readonly unknown[])[] = [];
		const invalidateQueries = async ({ queryKey }: { queryKey: readonly unknown[] }) => {
			invalidated.push(queryKey);
		};
		const channel = createMockChannel<PodExecSessionMessage>(() => {});

		await startPodExec({
			client,
			request: {
				clusterContext: "kind-dev",
				namespace: "payments",
				podName: "api-0",
				command: ["/bin/sh"],
				stdin: true,
				tty: true,
				terminalSize: { cols: 100, rows: 32 },
				confirmation: {
					acknowledged: true,
					target: "kind-dev/payments/Pod/api-0/container/<default>",
					command: '["/bin/sh"]',
				},
			},
			channel,
			invalidateQueries,
		});
		await stopPodExec({ client, sessionId: started.id, invalidateQueries });

		expect(calls).toEqual(["start", "stop"]);
		expect(invalidated).toEqual([
			["pod-exec-sessions"],
			["pod-exec-sessions"],
		]);
	});
});

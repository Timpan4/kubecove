import type { PodExecSessionSummary, PortForwardSessionSummary } from "@/lib/types";
import { createWorkspaceRecord } from "@/lib/workspace-model";
import { buildLiveSessionReadModel } from "./liveSessionReadModel";

declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toEqual(expected: unknown): void;
};

test("builds discriminated sorted live-session counts", () => {
	const portForward = {
		id: "port-1",
		clusterContext: "kind-dev",
		namespace: "payments",
		targetKind: "Pod",
		targetName: "api-0",
		podName: "api-0",
		remotePort: 8080,
		resolvedPodName: "api-0",
		resolvedPodPort: 8080,
		localPort: 18080,
		localAddress: "127.0.0.1",
		localUrl: "http://127.0.0.1:18080",
		status: "listening",
		startedAt: "2026-07-10T00:01:00Z",
	} satisfies PortForwardSessionSummary;
	const podExec = {
		id: "exec-1",
		clusterContext: "kind-dev",
		namespace: "payments",
		podName: "api-0",
		command: ["/bin/sh"],
		stdin: true,
		tty: true,
		terminalCols: 100,
		terminalRows: 32,
		status: "running",
		startedAt: "2026-07-10T00:00:00Z",
	} satisfies PodExecSessionSummary;

	const model = buildLiveSessionReadModel([portForward], [podExec]);

	expect(model.counts).toEqual({ portForwards: 1, podExec: 1, total: 2 });
	expect(model.items.map((item) => [item.kind, item.session.id])).toEqual([
		["podExec", "exec-1"],
		["portForward", "port-1"],
	]);
});

test("owns workspace and kubeconfig-source filtering for display arrays", () => {
	const workspace = createWorkspaceRecord({
		name: "Ops",
		clusterContext: "kind-dev",
		namespaces: [],
	});
	const portForward = {
		id: "port-1",
		clusterContext: "kind-dev",
		kubeconfigSourceKey: "kubeconfigSource=default",
		namespace: "payments",
		targetKind: "Pod",
		targetName: "api-0",
		podName: "api-0",
		remotePort: 8080,
		resolvedPodName: "api-0",
		resolvedPodPort: 8080,
		localPort: 18080,
		localAddress: "127.0.0.1",
		localUrl: "http://127.0.0.1:18080",
		status: "listening",
		startedAt: "2026-07-10T00:01:00Z",
	} satisfies PortForwardSessionSummary;
	const podExec = {
		id: "exec-1",
		clusterContext: "kind-dev",
		kubeconfigSourceKey: "kubeconfigSource=other",
		namespace: "payments",
		podName: "api-0",
		command: ["/bin/sh"],
		stdin: true,
		tty: true,
		terminalCols: 100,
		terminalRows: 32,
		status: "running",
		startedAt: "2026-07-10T00:00:00Z",
	} satisfies PodExecSessionSummary;

	const model = buildLiveSessionReadModel([portForward], [podExec], {
		workspace,
		kubeconfigSource: "kubeconfigSource=default",
	});

	expect(model.items.map((item) => item.session.id)).toEqual(["port-1"]);
	expect(model.portForwards.map((item) => item.id)).toEqual(["port-1"]);
	expect(model.podExecSessions).toEqual([]);
});

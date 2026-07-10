import { createMockTauriClient } from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import { createWorkspaceRecord } from "@/lib/workspace-model";
import {
	portForwardQueryOptions,
	portForwardSessionsForWorkspace,
	startSavedPortForward,
	stopPortForward,
} from "./portForwardLifecycle";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void | Promise<void>): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function session(
	id: string,
	clusterContext: string,
	kubeconfigSourceKey = "kubeconfigSource=default",
): PortForwardSessionSummary {
	return {
		id,
		clusterContext,
		kubeconfigSourceKey,
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
		startedAt: "2026-07-10T00:00:00Z",
	};
}

describe("port forward lifecycle", () => {
	test("owns sorted polling query options", async () => {
		const client = createMockTauriClient({
			list_port_forwards: [session("second", "kind-dev"), session("first", "kind-dev")],
		});
		const options = portForwardQueryOptions(client, {
			enabled: true,
			refetchInterval: 2_500,
		});

		expect(options.queryKey).toEqual(["port-forwards"]);
		expect(options.enabled).toBe(true);
		expect(options.refetchInterval).toBe(2_500);
		expect((await options.queryFn()).map((item) => item.id)).toEqual(["first", "second"]);
	});

	test("filters sessions by workspace contexts and kubeconfig source", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			clusterContexts: ["kind-dev", "prod"],
			namespaces: [],
		});
		const sessions = [
			session("dev", "kind-dev"),
			session("prod", "prod"),
			session("other-source", "kind-dev", "kubeconfigSource=other"),
			session("other-context", "staging"),
		];

		expect(
			portForwardSessionsForWorkspace(
				sessions,
				workspace,
				"kubeconfigSource=default",
			).map((item) => item.id),
		).toEqual(["dev", "prod"]);
	});

	test("invalidates the shared query after stopping", async () => {
		const calls: string[] = [];
		const client = createMockTauriClient({
			stop_port_forward: ({ sessionId }: Record<string, unknown>) => {
				calls.push(String(sessionId));
				return true;
			},
		});
		let invalidated: readonly unknown[] | null = null;

		await stopPortForward({
			client,
			sessionId: "port-forward-1",
			invalidateQueries: async ({ queryKey }) => {
				invalidated = queryKey;
			},
		});

		expect(calls).toEqual(["port-forward-1"]);
		expect(invalidated).toEqual(["port-forwards"]);
	});

	test("does not start a saved forward when active sessions cannot be listed", async () => {
		const client = createMockTauriClient({
			list_port_forwards: () => {
				throw new Error("session list unavailable");
			},
		});
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		let error: unknown = null;

		try {
			await startSavedPortForward({
				client,
				workspaceId: workspace.id,
				portForward: {
					id: "saved-1",
					clusterContext: "kind-dev",
					namespace: "payments",
					serviceName: "api",
					servicePort: 8080,
					createdAt: "2026-07-10T00:00:00Z",
					updatedAt: "2026-07-10T00:00:00Z",
				},
				updateSavedPortForward: () => {},
				invalidateQueries: async () => {},
			});
		} catch (caught) {
			error = caught;
		}

		expect(error instanceof Error ? error.message : error).toBe(
			"session list unavailable",
		);
	});
});

import { createMockTauriClient } from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import { createWorkspaceRecord } from "@/lib/workspace-model";
import {
	parseSavedPortForwardForWorkspace,
	portForwardQueryOptions,
	portForwardSessionsForWorkspace,
	savedPortForwardStartFailureMessage,
	shouldAutoStartSavedPortForwards,
	shouldShowSavedPortForwardRestorePrompt,
	startSavedPortForward,
	startSavedPortForwards,
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

	test("validates saved forward forms against workspace scope", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			clusterContexts: ["kind-dev", "prod"],
			namespaces: ["payments"],
		});
		const values = {
			clusterContext: "staging",
			namespace: "payments",
			serviceName: "api",
			servicePort: "8080",
			localPort: "18080",
			label: "API",
		};

		expect(parseSavedPortForwardForWorkspace(values, workspace)).toBe(
			"Cluster context must be in the current workspace scope.",
		);
		expect(
			parseSavedPortForwardForWorkspace(
				{ ...values, clusterContext: "prod" },
				workspace,
			),
		).toEqual({
			clusterContext: "prod",
			namespace: "payments",
			serviceName: "api",
			servicePort: 8080,
			localPort: 18080,
			label: "API",
		});
	});

	test("keeps restore eligibility scoped to caller-owned workspace state", () => {
		const workspace = createWorkspaceRecord({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		workspace.portForwards = [
			{
				id: "saved-1",
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: 8080,
				createdAt: "2026-07-10T00:00:00Z",
				updatedAt: "2026-07-10T00:00:00Z",
			},
		];

		expect(
			shouldShowSavedPortForwardRestorePrompt({
				workspace,
				autoStart: false,
				dismissedWorkspaceId: workspace.id,
			}),
		).toBe(false);
		expect(
			shouldShowSavedPortForwardRestorePrompt({
				workspace,
				autoStart: false,
				dismissedWorkspaceId: "another-workspace",
			}),
		).toBe(true);

		const startedWorkspaceIds = new Set<string>();
		expect(
			shouldAutoStartSavedPortForwards({
				workspace,
				autoStart: true,
				startedWorkspaceIds,
			}),
		).toBe(true);
		startedWorkspaceIds.add(workspace.id);
		expect(
			shouldAutoStartSavedPortForwards({
				workspace,
				autoStart: true,
				startedWorkspaceIds,
			}),
		).toBe(false);
	});

	test("summarizes restore conflicts before generic failures", () => {
		expect(
			savedPortForwardStartFailureMessage([
				{ ok: false, conflict: true },
				{ ok: false },
			]),
		).toBe(
			"1 saved forward has local port conflicts. Review port forwards for details.",
		);
		expect(savedPortForwardStartFailureMessage([{ ok: false }])).toBe(
			"1 saved forward failed to start. Review port forwards for details.",
		);
		expect(savedPortForwardStartFailureMessage([{ ok: true }])).toBe(null);
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

	test("does not bulk-start saved forwards when active sessions cannot be listed", async () => {
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
		workspace.portForwards = [
			{
				id: "saved-1",
				clusterContext: "kind-dev",
				namespace: "payments",
				serviceName: "api",
				servicePort: 8080,
				createdAt: "2026-07-10T00:00:00Z",
				updatedAt: "2026-07-10T00:00:00Z",
			},
		];
		let error: unknown = null;

		try {
			await startSavedPortForwards({
				client,
				workspace,
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

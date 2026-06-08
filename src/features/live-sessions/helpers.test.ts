import type { PortForwardSessionSummary } from "@/lib/types";
import type { SavedPortForward } from "@/lib/workspaces";
import {
	savedPortForwardLocalPortConflict,
	savedPortForwardMatchesSession,
} from "./helpers";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
};

function savedForward(overrides: Partial<SavedPortForward> = {}): SavedPortForward {
	return {
		id: "saved-1",
		clusterContext: "kind-dev",
		namespace: "payments",
		serviceName: "api",
		servicePort: 80,
		localPort: 18080,
		createdAt: "2026-06-01T00:00:00Z",
		updatedAt: "2026-06-01T00:00:00Z",
		lastStatus: "idle",
		...overrides,
	};
}

function session(
	overrides: Partial<PortForwardSessionSummary> = {},
): PortForwardSessionSummary {
	return {
		id: "port-forward-1",
		clusterContext: "other-context",
		kubeconfigSourceKey: "kubeconfigSource=abc123",
		namespace: "monitoring",
		targetKind: "Service",
		targetName: "grafana",
		podName: "grafana-0",
		remotePort: 80,
		resolvedPodName: "grafana-0",
		resolvedPodPort: 3000,
		localPort: 18080,
		localAddress: "127.0.0.1",
		localUrl: "http://127.0.0.1:18080",
		status: "listening",
		startedAt: "2026-06-01T00:00:00Z",
		...overrides,
	};
}

describe("saved port-forward conflict detection", () => {
	test("detects fixed local port held by another active session", () => {
		const conflicting = session();

		expect(
			savedPortForwardLocalPortConflict(savedForward(), [conflicting]),
		).toEqual(conflicting);
	});

	test("ignores auto local ports", () => {
		expect(
			savedPortForwardLocalPortConflict(
				savedForward({ localPort: undefined }),
				[session()],
			),
		).toBeNull();
	});

	test("matching saved forward remains reusable before conflict handling", () => {
		const matching = session({
			clusterContext: "kind-dev",
			namespace: "payments",
			targetName: "api",
			remotePort: 80,
		});

		expect(
			savedPortForwardMatchesSession(
				savedForward(),
				matching,
				"kubeconfigSource=abc123",
			),
		).toBe(true);
		expect(savedPortForwardLocalPortConflict(savedForward(), [matching])).toEqual(
			matching,
		);
	});
});

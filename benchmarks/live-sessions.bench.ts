import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import {
	findSavedPortForwardSession,
	sortPortForwardSessions,
} from "@/features/live-sessions/helpers";
import { buildLiveSessionReadModel } from "@/features/live-sessions/liveSessionReadModel";
import { sortPodExecSessions } from "@/features/live-sessions/podExecHelpers";
import type {
	PodExecSessionSummary,
	PortForwardSessionSummary,
} from "@/lib/types";
import type { SavedPortForward } from "@/lib/workspace-model";
import { savedWorkspace } from "./workspace-benchmark-fixtures";

const kubeconfigSourceKey = "kubeconfigSource=production";
const savedForwards: SavedPortForward[] = Array.from(
	{ length: 100 },
	(_, index) => {
		const forward: SavedPortForward = {
			id: `saved-${index}`,
			clusterContext: `prod/context-${index % 3}`,
			namespace: `namespace-${index % 100}`,
			serviceName: `service-${index}`,
			servicePort: 80 + (index % 3),
			label: `Saved service ${index}`,
			createdAt: "2026-01-01T00:00:00Z",
			updatedAt: "2026-02-01T00:00:00Z",
			lastStatus: index % 9 === 0 ? "error" : "listening",
		};
		if (index % 2 === 0) forward.localPort = 30_000 + index;
		return forward;
	},
);
const workspace = { ...savedWorkspace(0), portForwards: savedForwards };
const portForwardSessions: PortForwardSessionSummary[] = Array.from(
	{ length: 500 },
	(_, index) => ({
		id: `forward-${index}`,
		clusterContext: `prod/context-${index % 3}`,
		kubeconfigSourceKey,
		namespace: `namespace-${index % 100}`,
		targetKind: index % 2 === 0 ? "Service" : "Pod",
		targetName: index % 2 === 0 ? `service-${index % 100}` : `pod-${index}`,
		podName: `pod-${index}`,
		remotePort: 80 + (index % 3),
		resolvedPodName: `pod-${index}`,
		resolvedPodPort: 8080,
		localPort: 30_000 + (index % 200),
		localAddress: "127.0.0.1",
		localUrl: `http://127.0.0.1:${30_000 + (index % 200)}`,
		status:
			index % 11 === 0
				? "connected"
				: index % 7 === 0
					? "reconnecting"
					: "listening",
		startedAt: `2026-10-11T12:${String(Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}Z`,
	}),
);
const podExecSessions: PodExecSessionSummary[] = Array.from(
	{ length: 250 },
	(_, index) => ({
		id: `exec-${index}`,
		clusterContext: `prod/context-${index % 3}`,
		kubeconfigSourceKey,
		namespace: `namespace-${index % 100}`,
		podName: `pod-${index}`,
		container: "app",
		command: ["sh", "-lc", "sleep 3600"],
		stdin: true,
		tty: true,
		terminalCols: 120,
		terminalRows: 36,
		status:
			index % 11 === 0
				? "starting"
				: index % 5 === 0
					? "connecting"
					: "running",
		startedAt: `2026-10-11T11:${String(Math.floor(index / 60)).padStart(2, "0")}:${String(index % 60).padStart(2, "0")}Z`,
	}),
);
const sortedForwards = sortPortForwardSessions(portForwardSessions);
const sortedExec = sortPodExecSessions(podExecSessions);
const visibleReadModel = buildLiveSessionReadModel(sortedForwards, sortedExec, {
	workspace,
	kubeconfigSource: kubeconfigSourceKey,
});
const matchedSavedRows = savedForwards.filter((saved) =>
	findSavedPortForwardSession(
		saved,
		visibleReadModel.portForwards,
		kubeconfigSourceKey,
	),
).length;

assert.equal(portForwardSessions.length, 500);
assert.equal(podExecSessions.length, 250);
assert.equal(savedForwards.length, 100);
assert(visibleReadModel.items.length > 0);
assert(matchedSavedRows > 0);
const firstForward = portForwardSessions[0];
const firstPreset = savedForwards[0];
const lastForward = visibleReadModel.portForwards.at(-1);
assert(firstForward && firstPreset && lastForward);
const latePreset: SavedPortForward = {
	...firstPreset,
	clusterContext: lastForward.clusterContext,
	namespace: lastForward.namespace,
	serviceName: "late-service",
	servicePort: lastForward.remotePort,
	localPort: lastForward.localPort,
};
const lateForward = {
	...lastForward,
	targetKind: "Service",
	targetName: latePreset.serviceName,
};
const lateLookupRows = [
	...visibleReadModel.portForwards,
	lateForward,
	{ ...lateForward, id: "later-duplicate" },
];
assert.equal(
	findSavedPortForwardSession(
		firstPreset,
		[firstForward, { ...firstForward, id: "duplicate" }],
		kubeconfigSourceKey,
	),
	firstForward,
);
assert.equal(
	findSavedPortForwardSession(latePreset, lateLookupRows, kubeconfigSourceKey),
	lateForward,
);
assert.equal(
	findSavedPortForwardSession(
		latePreset,
		lateLookupRows,
		"kubeconfigSource=other",
	),
	undefined,
);
assert.equal(
	findSavedPortForwardSession(
		{ ...latePreset, namespace: "missing" },
		lateLookupRows,
		kubeconfigSourceKey,
	),
	undefined,
);
assert.equal(
	visibleReadModel.items.reduce(
		(count, item) =>
			count +
			(item.session.status === "connected" || item.session.status === "starting"
				? 1
				: 0),
		0,
	) > 0,
	true,
);

describe("live session read model F58", () => {
	bench(
		"F58 sort returned sessions, filter workspace contexts, combine, and split display rows",
		() => {
			const forwards = sortPortForwardSessions(portForwardSessions);
			const execSessions = sortPodExecSessions(podExecSessions);
			buildLiveSessionReadModel(forwards, execSessions, {
				workspace,
				kubeconfigSource: kubeconfigSourceKey,
			});
		},
	);

	bench(
		"F59 first-match saved Service forward lookup for each mounted preset",
		() => {
			for (const saved of savedForwards) {
				findSavedPortForwardSession(
					saved,
					visibleReadModel.portForwards,
					kubeconfigSourceKey,
				);
			}
			findSavedPortForwardSession(
				latePreset,
				lateLookupRows,
				kubeconfigSourceKey,
			);
		},
	);
});

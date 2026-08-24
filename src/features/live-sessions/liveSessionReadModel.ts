import type {
	PodExecSessionSummary,
	PortForwardSessionSummary,
} from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspace-model";
import { podExecSessionsForWorkspace } from "./podExecLifecycle";
import { portForwardSessionsForWorkspace } from "./portForwardLifecycle";

export type LiveSessionReadItem =
	| { kind: "portForward"; session: PortForwardSessionSummary }
	| { kind: "podExec"; session: PodExecSessionSummary };

export interface LiveSessionReadModel {
	portForwards: PortForwardSessionSummary[];
	podExecSessions: PodExecSessionSummary[];
	items: LiveSessionReadItem[];
	counts: {
		portForwards: number;
		podExec: number;
		total: number;
	};
}

export function buildLiveSessionReadModel(
	portForwards: PortForwardSessionSummary[],
	podExecSessions: PodExecSessionSummary[],
	options?: {
		workspace: SavedWorkspace;
		kubeconfigSource?: string;
	},
): LiveSessionReadModel {
	const visiblePortForwards = options
		? portForwardSessionsForWorkspace(
				portForwards,
				options.workspace,
				options.kubeconfigSource,
			)
		: portForwards;
	const visiblePodExecSessions = options
		? podExecSessionsForWorkspace(
				podExecSessions,
				options.workspace,
				options.kubeconfigSource,
			)
		: podExecSessions;
	const items: LiveSessionReadItem[] = [];
	for (const session of visiblePortForwards) items.push({ kind: "portForward", session });
	for (const session of visiblePodExecSessions) items.push({ kind: "podExec", session });
	items.sort((a, b) => a.session.startedAt.localeCompare(b.session.startedAt));

	const sortedPortForwards: PortForwardSessionSummary[] = [];
	const sortedPodExecSessions: PodExecSessionSummary[] = [];
	for (const item of items) {
		if (item.kind === "portForward") sortedPortForwards.push(item.session);
		else sortedPodExecSessions.push(item.session);
	}

	return {
		portForwards: sortedPortForwards,
		podExecSessions: sortedPodExecSessions,
		items,
		counts: {
			portForwards: sortedPortForwards.length,
			podExec: sortedPodExecSessions.length,
			total: items.length,
		},
	};
}

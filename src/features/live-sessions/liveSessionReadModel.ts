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
	const items: LiveSessionReadItem[] = [
		...visiblePortForwards.map((session) => ({ kind: "portForward" as const, session })),
		...visiblePodExecSessions.map((session) => ({ kind: "podExec" as const, session })),
	].toSorted((a, b) => a.session.startedAt.localeCompare(b.session.startedAt));
	const sortedPortForwards = items.flatMap((item) =>
		item.kind === "portForward" ? [item.session] : [],
	);
	const sortedPodExecSessions = items.flatMap((item) =>
		item.kind === "podExec" ? [item.session] : [],
	);

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

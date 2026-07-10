import type {
	PodExecSessionSummary,
	PortForwardSessionSummary,
} from "@/lib/types";

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
): LiveSessionReadModel {
	const items: LiveSessionReadItem[] = [
		...portForwards.map((session) => ({ kind: "portForward" as const, session })),
		...podExecSessions.map((session) => ({ kind: "podExec" as const, session })),
	].toSorted((a, b) => a.session.startedAt.localeCompare(b.session.startedAt));

	return {
		portForwards,
		podExecSessions,
		items,
		counts: {
			portForwards: portForwards.length,
			podExec: podExecSessions.length,
			total: items.length,
		},
	};
}

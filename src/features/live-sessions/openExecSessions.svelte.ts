import { SvelteMap } from "svelte/reactivity";

// Exec sessions live only while their Pod's Exec tab stays open. Open tabs register
// their running sessions here so actions that would close the tab can ask first,
// and the Live sessions view can bring a session's tab back into view.
export interface OpenExecSession {
	id: string;
	podName: string;
	container: string;
	command: readonly string[];
}

interface OpenExecTab {
	sessions: () => OpenExecSession[];
	show: (sessionId: string) => void;
	/** Resolves false when any session could not be stopped. */
	stopAll: () => Promise<boolean>;
}

interface StopPrompt {
	sessions: OpenExecSession[];
	resolve: (stop: boolean) => void;
}

const openTabs = new SvelteMap<symbol, OpenExecTab>();
let stopPrompt = $state<StopPrompt | null>(null);

export function registerOpenExecSessions(tab: OpenExecTab): () => void {
	const key = Symbol("exec-tab");
	openTabs.set(key, tab);
	return () => openTabs.delete(key);
}

export function openExecSessions(): OpenExecSession[] {
	return [...openTabs.values()].flatMap((tab) => tab.sessions());
}

export function showOpenExecSession(sessionId: string): void {
	for (const tab of openTabs.values()) {
		if (tab.sessions().some((session) => session.id === sessionId)) {
			tab.show(sessionId);
			return;
		}
	}
}

export function hasOpenExecSessions(): boolean {
	return openExecSessions().length > 0;
}

/**
 * Resolves true when no exec session is open. Otherwise asks first; on agreement
 * every open session is stopped before resolving, so chained leave actions do not
 * prompt twice. Resolves false when a stop fails, keeping the user on the Pod
 * where that tab shows the error.
 */
export function confirmStoppingExecSessions(): Promise<boolean> {
	const sessions = openExecSessions();
	if (sessions.length === 0) return Promise.resolve(true);
	stopPrompt?.resolve(false);
	return new Promise((resolve) => {
		stopPrompt = {
			sessions,
			resolve: (stop) => {
				if (!stop) {
					resolve(false);
					return;
				}
				void Promise.all([...openTabs.values()].map((tab) => tab.stopAll())).then((stopped) =>
					resolve(stopped.every(Boolean)),
				);
			},
		};
	});
}

export const pendingStopPrompt = {
	get current() {
		return stopPrompt;
	},
};

export function answerStopPrompt(stop: boolean): void {
	const prompt = stopPrompt;
	stopPrompt = null;
	prompt?.resolve(stop);
}

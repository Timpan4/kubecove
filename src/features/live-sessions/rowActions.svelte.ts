import { SvelteMap } from "svelte/reactivity";

export type LiveSessionRowAction = "stop" | "copy" | "reconnect" | "start";

export interface LiveSessionRowFailure {
	action: LiveSessionRowAction;
	error: unknown;
}

/** Saved forwards share a tracker with live sessions, so their ids get their own namespace. */
export function savedForwardRowId(savedForwardId: string): string {
	return `saved:${savedForwardId}`;
}

/**
 * Tracks one in-flight action per row id plus the latest failure. A single
 * id slot per action let one finishing action clear another row's spinner.
 */
export class LiveSessionRowActions {
	readonly #pending = new SvelteMap<string, LiveSessionRowAction>();
	failure = $state<LiveSessionRowFailure | null>(null);

	pending(id: string): LiveSessionRowAction | undefined {
		return this.#pending.get(id);
	}

	/** Runs the task unless the row is busy. Resolves to true when it succeeded. */
	async run<Result>(
		id: string,
		action: LiveSessionRowAction,
		task: () => Promise<Result>,
	): Promise<boolean> {
		if (this.#pending.has(id)) return false;
		this.#pending.set(id, action);
		this.failure = null;
		try {
			await task();
			return true;
		} catch (error) {
			this.failure = { action, error };
			return false;
		} finally {
			this.#pending.delete(id);
		}
	}
}

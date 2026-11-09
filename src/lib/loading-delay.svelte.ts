import { untrack } from "svelte";

// The user chose both values. `src-tauri/gpui/src/inspector/loading.rs` uses the same ones.
/** A loading indicator appears only when its read is still pending after this delay. */
export const LOADING_SHOW_DELAY_MS = 300;
/** Once shown, a loading indicator stays visible for at least this long. */
export const LOADING_MIN_VISIBLE_MS = 300;

export type LoadingIndicator = {
	/** Render the indicator. */
	readonly shown: boolean;
	/** The read is pending or its indicator is held. Content waits while this is true. */
	readonly active: boolean;
};

/** "waiting": pending, before the show delay. "shown": visible, and held after the read settles. */
class LoadingTimer {
	phase: "idle" | "waiting" | "shown" = "idle";
	private shownAt = 0;
	private timer: ReturnType<typeof setTimeout> | undefined;

	constructor(private readonly changed: () => void) {}

	update(pending: boolean): void {
		if (pending) {
			// A new read during the minimum visible time keeps the indicator.
			if (this.phase === "shown") this.dispose();
			else if (this.timer === undefined) {
				this.phase = "waiting";
				this.timer = setTimeout(() => {
					this.timer = undefined;
					this.shownAt = performance.now();
					this.set("shown");
				}, LOADING_SHOW_DELAY_MS);
			}
		} else if (this.phase === "waiting") {
			this.dispose();
			this.phase = "idle";
		} else if (this.phase === "shown" && this.timer === undefined) {
			const remaining = this.shownAt + LOADING_MIN_VISIBLE_MS - performance.now();
			if (remaining <= 0) this.set("idle");
			else
				this.timer = setTimeout(() => {
					this.timer = undefined;
					this.set("idle");
				}, remaining);
		}
	}

	dispose(): void {
		clearTimeout(this.timer);
		this.timer = undefined;
	}

	private set(phase: "idle" | "shown"): void {
		this.phase = phase;
		this.changed();
	}
}

/**
 * Times a loading indicator for the read that `pending` reports. The read itself
 * starts at once; only the indicator and the content it replaces wait. Call during
 * component initialization.
 */
export function loadingDelay(pending: () => boolean): LoadingIndicator {
	let version = $state(0);
	const timer = new LoadingTimer(() => version++);
	$effect(() => {
		const isPending = pending();
		untrack(() => timer.update(isPending));
	});
	$effect(() => () => timer.dispose());
	return {
		get shown() {
			void version;
			return timer.phase === "shown";
		},
		get active() {
			void version;
			return timer.phase === "shown" || pending();
		},
	};
}

/** Per-key `LoadingIndicator` fields for a set of keyed reads. */
export interface KeyedLoadingIndicator {
	shown(key: string): boolean;
	active(key: string): boolean;
}

/** `loadingDelay` for a set of keyed reads, such as one per namespace. */
export function loadingDelays(pending: () => ReadonlySet<string>): KeyedLoadingIndicator {
	let version = $state(0);
	const timers = new Map<string, LoadingTimer>();
	$effect(() => {
		const keys = pending();
		untrack(() => {
			for (const key of keys) if (!timers.has(key)) timers.set(key, new LoadingTimer(() => version++));
			for (const [key, timer] of timers) {
				timer.update(keys.has(key));
				if (timer.phase === "idle") timers.delete(key);
			}
		});
	});
	$effect(() => () => {
		for (const timer of timers.values()) timer.dispose();
	});
	const shown = (key: string) => {
		void version;
		return timers.get(key)?.phase === "shown";
	};
	return { shown, active: (key) => shown(key) || pending().has(key) };
}

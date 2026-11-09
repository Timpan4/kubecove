import type { QueryClient } from "@tanstack/svelte-query";
import type { QueryMatch } from "./queryKeys";
import { refreshCurrentView } from "./resource-refresh";
import {
	observeResourceScope,
	RESOURCE_RECOVERY_INTERVAL,
	type ResourceScopeHandlers,
	type ResourceScopeObserver,
	type ResourceWatchState,
	watchKeySignature,
} from "./resource-watch";
import type { TauriClient } from "./tauri-runtime";
import type { createStreamChannel } from "./tauri-streams";
import type { WatchResourceKey } from "./types";
import { messageFromError } from "./error-redaction";

// A view declares which watched kinds keep which reads fresh. The binding owns the
// watch streams, per-read throttling, live status, recovery reloads, and manual
// refresh. Its watches stop with the view and never with finite-read generation
// rotation (ADR 0009).

/** Reads that a change to one of `keys` makes stale, selected by a query catalog matcher. */
export interface LiveRead {
	keys: readonly WatchResourceKey[];
	/**
	 * Kinds whose changes also make this read stale although it shows no rows
	 * read from them, such as the resources an Argo CD Application manages. Their
	 * watches start at the versions of the reads that show their rows.
	 */
	refreshedBy?: readonly WatchResourceKey[];
	match: QueryMatch;
	/**
	 * False marks the reads stale without refetching them, for reads the view is
	 * not showing or that another live view keeps fresh.
	 */
	refetch?: boolean;
	/**
	 * The list version of the rows this read shows, by watch key signature, so a
	 * watch starts at those rows. Null while the rows load. A key without an
	 * entry has no version. Only a read whose rows the view shows declares
	 * versions, so they apply even when another live view refetches it.
	 */
	versions?: ReadonlyMap<string, string | null>;
}

/** `versions` for a read of one versioned list: null while it loads, no entry when it reported none. */
export function listReadVersions(
	key: WatchResourceKey,
	read: { isPending: boolean; data?: { resourceVersion?: string } },
): Map<string, string | null> {
	const version = read.isPending ? null : read.data?.resourceVersion;
	return new Map(version === undefined ? [] : [[watchKeySignature(key), version]]);
}

export interface LiveView {
	clusterContext: string;
	kubeconfigSourceKey?: string;
	/** Namespaces whose cached backend lists a refresh evicts. */
	namespaces: readonly string[];
	/** Every watched kind. A kind without a read still reports live status. */
	keys: readonly WatchResourceKey[];
	reads: readonly LiveRead[];
	/** "view": a lost watch and manual refresh reread every active read in the view; "reads": only the declared reads. */
	refreshScope: "view" | "reads";
}

export interface LiveViewState {
	catchingUp: boolean;
	status: ResourceWatchState["status"] | "idle";
	message: string;
	error: string | null;
	reloadError: string | null;
	warnings: string[];
}

export const IDLE_LIVE_VIEW: LiveViewState = {
	catchingUp: false,
	status: "idle",
	message: "No live resource watches in this view.",
	error: null,
	reloadError: null,
	warnings: [],
};

const STATUS_SEVERITY: LiveViewState["status"][] = ["reconnecting", "connecting", "partial", "polling", "connected", "idle"];

/**
 * One state for several contexts' views: the most severe status, except that a
 * context that polls next to live ones makes the whole view partial. With more
 * than one state, messages, errors and warnings name their context.
 */
export function combineLiveViewStates(states: readonly { clusterContext: string; state: LiveViewState }[]): LiveViewState {
	if (states.length <= 1) return states[0]?.state ?? IDLE_LIVE_VIEW;
	const named = (context: string, text: string | null) => (text === null ? null : `${context}: ${text}`);
	const active = states.filter(({ state }) => state.status !== "idle");
	if (active.length === 0) return IDLE_LIVE_VIEW;
	const [worst] = [...active].sort((left, right) =>
		STATUS_SEVERITY.indexOf(left.state.status) - STATUS_SEVERITY.indexOf(right.state.status));
	const mixedPolling = worst.state.status === "polling" && active.some(({ state }) => state.status !== "polling");
	return {
		status: mixedPolling ? "partial" : worst.state.status,
		catchingUp: active.some(({ state }) => state.catchingUp),
		message: named(worst.clusterContext, worst.state.message) ?? "",
		error: active.map(({ clusterContext, state }) => named(clusterContext, state.error)).find(Boolean) ?? null,
		reloadError: active.map(({ clusterContext, state }) => named(clusterContext, state.reloadError)).find(Boolean) ?? null,
		warnings: active.flatMap(({ clusterContext, state }) => state.warnings.map((warning) => `${clusterContext}: ${warning}`)),
	};
}

/** A component's live view: its state, the identity of its watched scope, and manual refresh. */
export interface LiveViewControls {
	readonly state: LiveViewState;
	readonly key: string;
	refresh(): Promise<string[]>;
}

// A watch can change after a running list took its snapshot. Keep one trailing
// refresh, even when that list takes longer than the watch coalescing interval.
export function createWatchRefreshThrottle(refresh: () => Promise<void>) {
	let disposed = false;
	let pending = false;
	let inFlight = false;
	let nextStart = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;
	async function run() {
		timer = undefined;
		if (disposed || inFlight || !pending) return;
		pending = false;
		inFlight = true;
		nextStart = Date.now() + 250;
		try {
			await refresh();
		} catch {
			/* Query state owns list errors. Later watch events can retry. */
		} finally {
			inFlight = false;
			schedule();
		}
	}
	function schedule() {
		if (disposed || inFlight || timer || !pending) return;
		const delay = nextStart - Date.now();
		if (delay <= 0) void run();
		else timer = setTimeout(() => void run(), delay);
	}
	return {
		notify() {
			if (!disposed) {
				pending = true;
				schedule();
			}
		},
		dispose() {
			disposed = true;
			if (timer) clearTimeout(timer);
		},
	};
}

/** Throttled refresh of the reads `match()` selects, read at each refresh. A change seen while one of them runs refreshes them again once it ends. */
function createReadRefresh(queryClient: QueryClient, match: () => QueryMatch, beforeRefresh?: () => void) {
	const throttle = createWatchRefreshThrottle(async () => {
		const filter = { predicate: match() };
		const alreadyFetching = queryClient.isFetching({ ...filter, type: "active" }) > 0;
		beforeRefresh?.();
		await queryClient.invalidateQueries(filter, { cancelRefetch: false });
		if (alreadyFetching) throttle.notify();
	});
	return throttle;
}

/** Throttled refresh of the reads `match` selects. A change seen while one of them runs refreshes them again once it ends. */
export function createLiveReadRefresh(queryClient: QueryClient, match: QueryMatch, beforeRefresh?: () => void) {
	const refresh = createReadRefresh(queryClient, () => match, beforeRefresh);
	let timer: ReturnType<typeof setTimeout> | undefined;
	let disposed = false;
	const failedRead = () => queryClient.getQueryCache().getAll().some((query) =>
		match(query) && query.isActive() && query.state.status === "error" && query.state.fetchStatus === "idle");
	function checkRecovery() {
		if (disposed) return;
		if (!failedRead()) {
			if (timer !== undefined) clearTimeout(timer);
			timer = undefined;
		} else if (timer === undefined) {
			timer = setTimeout(() => {
				timer = undefined;
				if (!disposed && failedRead()) refresh.notify();
			}, RESOURCE_RECOVERY_INTERVAL);
		}
	}
	const unsubscribe = queryClient.getQueryCache().subscribe(({ query }) => {
		if (match(query)) checkRecovery();
	});
	checkRecovery();
	return {
		notify: refresh.notify,
		dispose() {
			disposed = true;
			if (timer !== undefined) clearTimeout(timer);
			unsubscribe();
			refresh.dispose();
		},
	};
}

// A key that names only an apiVersion matches a discovered kind that also names its group.
function watchKeyMatches(key: WatchResourceKey, candidate: WatchResourceKey): boolean {
	return (
		key.resourceKind.kind === candidate.resourceKind.kind &&
		watchGroup(key) === watchGroup(candidate) &&
		(!key.namespace || !candidate.namespace || key.namespace === candidate.namespace)
	);
}

/**
 * The version a watch on `key` starts at: the version every shown read it keeps
 * fresh lists, null while one of them loads, or undefined when they have none
 * or disagree. Starting at the oldest rows shown would need ordered versions,
 * which Kubernetes does not promise. A read that `key` only refreshes
 * (`refreshedBy`) shows no rows of it, so it does not vote.
 */
function startVersion(view: LiveView, key: WatchResourceKey): string | null | undefined {
	const signature = watchKeySignature(key);
	const shown = view.reads
		.filter((read) => (read.refetch !== false || read.versions) && read.keys.some((candidate) => watchKeyMatches(candidate, key)))
		.map((read) => read.versions?.get(signature));
	if (shown.includes(null)) return null;
	const [first] = shown;
	return shown.every((version) => version === first) ? first : undefined;
}

function watchGroup(key: WatchResourceKey): string {
	const { group, apiVersion } = key.resourceKind;
	return group ?? (apiVersion?.includes("/") ? apiVersion.slice(0, apiVersion.indexOf("/")) : "");
}

/** A declared read's refresh. The throttle outlives read-set changes, so a pending refresh still runs for what the view shows now. */
function createReadSlot(queryClient: QueryClient, initial: LiveRead) {
	let read = initial;
	const throttle = createReadRefresh(queryClient, () => read.match);
	return {
		get read() {
			return read;
		},
		set(next: LiveRead) {
			read = next;
		},
		notify() {
			if (read.refetch === false) void queryClient.invalidateQueries({ predicate: read.match, refetchType: "none" });
			else throttle.notify();
		},
		dispose: () => throttle.dispose(),
	};
}

export interface LiveViewBinding {
	refresh(): Promise<string[]>;
	/** Applies a changed view with the same owner. Watch keys the view keeps keep their streams. */
	update(view: LiveView): void;
	stop(): void;
}

/** Binds a view's watches and refresh paths. The owner (cluster context and kubeconfig source) must not change; bind again for a new owner. */
export function bindLiveView({
	client,
	queryClient,
	view,
	onState,
	openChannel,
}: {
	client: TauriClient;
	queryClient: QueryClient;
	view: LiveView;
	onState: (state: LiveViewState) => void;
	openChannel?: typeof createStreamChannel;
}): LiveViewBinding {
	let current = view;
	let stopped = false;
	let observer: ResourceScopeObserver | null = null;
	// Slots are kept by declared position. A changed read set re-points each slot.
	const slots: ReturnType<typeof createReadSlot>[] = [];
	const keptFreshBy = (read: LiveRead, key: WatchResourceKey) => {
		const matches = (candidate: WatchResourceKey) => watchKeyMatches(candidate, key);
		return read.keys.some(matches) || (read.refreshedBy?.some(matches) ?? false);
	};
	const refetched = () => current.reads.filter((read) => read.refetch !== false);
	const viewReads = () => (current.refreshScope === "view" ? undefined : refetched().map((read) => read.match));
	const reread = (keys: readonly WatchResourceKey[], matches: readonly QueryMatch[] | undefined, cancelRefetch: boolean) =>
		refreshCurrentView({
			client,
			queryClient,
			clusterContext: current.clusterContext,
			kubeconfigEnvVar: current.kubeconfigSourceKey,
			keys: [...keys],
			namespaces: [...current.namespaces],
			reads: matches,
			cancelRefetch,
		});
	// One handler set for the binding's lifetime; it reads the current view when it runs.
	const handlers: ResourceScopeHandlers = {
		readError: () => {
			const failed = queryClient.getQueryCache().findAll({
				predicate: (query) => query.isActive() && query.state.status === "error"
					&& refetched().some((read) => read.match(query)),
			})[0];
			return failed ? messageFromError(failed.state.error) : null;
		},
		onState: (state) => onState({ ...state, reloadError: state.reloadError ?? null, warnings: state.warnings ?? [] }),
		startVersion: (key) => startVersion(current, key),
		onChange: (_event, key) => {
			for (const slot of slots) if (keptFreshBy(slot.read, key)) slot.notify();
		},
		// Automatic recovery waits for running reads. Permission recovery rereads only
		// what the denied kinds keep fresh; a lost watch rereads the whole scope.
		reload: (denied) =>
			denied
				? reread(denied, refetched().filter((read) => denied.some((key) => keptFreshBy(read, key))).map((read) => read.match), false)
				: reread(current.keys, viewReads(), false),
	};
	const stopQueryUpdates = queryClient.getQueryCache().subscribe(({ query, type }) => {
		if (type === "updated" && current.reads.some((read) => read.refetch !== false && read.match(query))) {
			observer?.checkReads();
		}
	});
	function apply(next: LiveView) {
		current = next;
		next.reads.forEach((read, index) => {
			const slot = slots[index];
			if (slot) slot.set(read);
			else slots.push(createReadSlot(queryClient, read));
		});
		for (const slot of slots.splice(next.reads.length)) slot.dispose();
		if (next.keys.length === 0) {
			observer?.stop();
			observer = null;
			onState(IDLE_LIVE_VIEW);
		} else if (observer) {
			observer.update([...next.keys], handlers);
		} else {
			observer = observeResourceScope({
				client,
				clusterContext: next.clusterContext,
				keys: [...next.keys],
				kubeconfigEnvVar: next.kubeconfigSourceKey,
				openChannel,
				...handlers,
			});
		}
	}
	apply(view);
	return {
		refresh: () => reread(current.keys, viewReads(), true),
		update(next) {
			if (!stopped) apply(next);
		},
		stop() {
			stopped = true;
			stopQueryUpdates();
			observer?.stop();
			observer = null;
			for (const slot of slots.splice(0)) slot.dispose();
		},
	};
}

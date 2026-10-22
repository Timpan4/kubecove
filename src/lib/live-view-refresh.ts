import type { QueryClient } from "@tanstack/svelte-query";
import type { QueryMatch } from "./queryKeys";
import { refreshCurrentView } from "./resource-refresh";
import { observeResourceScope, type ResourceWatchState } from "./resource-watch";
import type { TauriClient } from "./tauri-runtime";
import type { createStreamChannel } from "./tauri-streams";
import type { WatchResourceKey } from "./types";

// A view declares which watched kinds keep which reads fresh. The binding owns the
// watch streams, per-read throttling, live status, recovery reloads, and manual
// refresh. Its watches stop with the view and never with finite-read generation
// rotation (ADR 0009).

/** Reads that a change to one of `keys` makes stale, selected by a query catalog matcher. */
export interface LiveRead {
	keys: readonly WatchResourceKey[];
	match: QueryMatch;
	/** False marks the reads stale without refetching them, for reads the view is not showing. */
	refetch?: boolean;
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
	status: ResourceWatchState["status"] | "idle";
	message: string;
	error: string | null;
	reloadError: string | null;
	warnings: string[];
}

export const IDLE_LIVE_VIEW: LiveViewState = {
	status: "idle",
	message: "No live resource watches in this view.",
	error: null,
	reloadError: null,
	warnings: [],
};

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

/** Throttled refresh of the reads `match` selects. A change seen while one of them runs refreshes them again once it ends. */
export function createLiveReadRefresh(queryClient: QueryClient, match: QueryMatch, beforeRefresh?: () => void) {
	const filter = { predicate: match };
	const throttle = createWatchRefreshThrottle(async () => {
		const alreadyFetching = queryClient.isFetching({ ...filter, type: "active" }) > 0;
		beforeRefresh?.();
		await queryClient.invalidateQueries(filter, { cancelRefetch: false });
		if (alreadyFetching) throttle.notify();
	});
	return throttle;
}

// A key that names only an apiVersion matches a discovered kind that also names its group.
function watchKeyMatches(key: WatchResourceKey, candidate: WatchResourceKey): boolean {
	return (
		key.resourceKind.kind === candidate.resourceKind.kind &&
		watchGroup(key) === watchGroup(candidate) &&
		(!key.namespace || !candidate.namespace || key.namespace === candidate.namespace)
	);
}

function watchGroup(key: WatchResourceKey): string {
	const { group, apiVersion } = key.resourceKind;
	return group ?? (apiVersion?.includes("/") ? apiVersion.slice(0, apiVersion.indexOf("/")) : "");
}

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
}) {
	const keptFreshBy = (read: LiveRead, key: WatchResourceKey) =>
		read.keys.some((candidate) => watchKeyMatches(candidate, key));
	const reads = view.reads.map((read) => ({
		read,
		refresh: read.refetch === false
			? { notify: () => void queryClient.invalidateQueries({ predicate: read.match, refetchType: "none" }), dispose() {} }
			: createLiveReadRefresh(queryClient, read.match),
	}));
	const refetched = view.reads.filter((read) => read.refetch !== false);
	const viewReads = view.refreshScope === "view" ? undefined : refetched.map((read) => read.match);
	const reread = (keys: readonly WatchResourceKey[], matches: readonly QueryMatch[] | undefined, cancelRefetch: boolean) =>
		refreshCurrentView({
			client,
			queryClient,
			clusterContext: view.clusterContext,
			kubeconfigEnvVar: view.kubeconfigSourceKey,
			keys: [...keys],
			namespaces: [...view.namespaces],
			reads: matches,
			cancelRefetch,
		});
	let stopWatches = () => {};
	if (view.keys.length === 0) onState(IDLE_LIVE_VIEW);
	else stopWatches = observeResourceScope({
		client,
		clusterContext: view.clusterContext,
		keys: [...view.keys],
		kubeconfigEnvVar: view.kubeconfigSourceKey,
		openChannel,
		onState: (state) => onState({ ...state, reloadError: state.reloadError ?? null, warnings: state.warnings ?? [] }),
		onChange: (_event, key) => {
			for (const { read, refresh } of reads) if (keptFreshBy(read, key)) refresh.notify();
		},
		// Automatic recovery waits for running reads. Permission recovery rereads only
		// what the denied kinds keep fresh; a lost watch rereads the whole scope.
		reload: (denied) =>
			denied
				? reread(denied, refetched.filter((read) => denied.some((key) => keptFreshBy(read, key))).map((read) => read.match), false)
				: reread(view.keys, viewReads, false),
	});
	return {
		refresh: () => reread(view.keys, viewReads, true),
		stop() {
			stopWatches();
			for (const { refresh } of reads) refresh.dispose();
		},
	};
}

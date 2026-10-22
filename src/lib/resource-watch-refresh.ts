import type { QueryClient, QueryKey } from "@tanstack/svelte-query";
import type { WatchResourceKey } from "./types";

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

export function createWatchQueryRefresh(
	queryClient: QueryClient,
	queryKey: QueryKey,
	exact = true,
) {
	const filter = { queryKey, exact };
	const throttle = createWatchRefreshThrottle(async () => {
		const alreadyFetching = queryClient.isFetching({ ...filter, type: "active" }) > 0;
		await queryClient.invalidateQueries(filter, { cancelRefetch: false });
		if (alreadyFetching) throttle.notify();
	});
	return throttle;
}

export function watchKeyMatches(
	key: WatchResourceKey,
	candidate: WatchResourceKey,
): boolean {
	return (
		key.resourceKind.kind === candidate.resourceKind.kind &&
		watchGroup(key) === watchGroup(candidate) &&
		(!key.namespace ||
			!candidate.namespace ||
			key.namespace === candidate.namespace)
	);
}

function watchGroup(key: WatchResourceKey): string {
	const { group, apiVersion } = key.resourceKind;
	return (
		group ??
		(apiVersion?.includes("/")
			? apiVersion.slice(0, apiVersion.indexOf("/"))
			: "")
	);
}

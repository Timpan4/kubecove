import type { QueryClient } from "@tanstack/svelte-query";
import type { ResourceSummary, StreamMessage } from "@/lib/types";
import { createLiveReadRefresh } from "@/lib/live-view-refresh";
import { queryMatchers, type QueryMatch } from "@/lib/queryKeys";
import { isTransientQueryError } from "@/lib/query-retry";
import { RESOURCE_RECOVERY_INTERVAL } from "@/lib/resource-watch";
import type { ResourceDetailReadSpec } from "./resourceDetailReadSpec";
import { diagnosticLog } from "@/lib/diagnostics";
import { messageFromError } from "@/lib/error-redaction";
import {
	closeStreamChannel,
	createStreamChannel,
	startResourceEventWatch,
	startResourceWatch,
	stopStream,
	type TauriClient,
} from "@/lib/tauri";

type ResourceWatch = Pick<ResourceSummary, "cluster" | "name" | "namespace">;
type EventWatch = Pick<ResourceDetailReadSpec, "eventWatch" | "eventsQueryKey">;

export function resourceDetailWatches(
	getClient: () => TauriClient,
	queryClient: QueryClient,
	resourceWatch: () => {
		resource: ResourceWatch;
		watchKey: ResourceDetailReadSpec["resourceWatchKey"];
		detailsQueryKey: ResourceDetailReadSpec["detailsQueryKey"];
		argoReads: QueryMatch | null;
		source: string | undefined;
	} | null,
	eventWatch: () => { key: string; source: string | undefined; read: EventWatch } | null,
	onResourceRefresh: () => void,
) {
	let realtimeWatchError = $state<string | null>(null);
	let realtimeEventsWatchError = $state<string | null>(null);
	let eventsWatchPhase = $state<"opening" | "connected" | "recovering">("opening");
	$effect(() => {
		const watch = resourceWatch();
		if (!watch) return;
		const client = getClient();
		const { resource, watchKey, detailsQueryKey, argoReads, source } = watch;
		let cancelled = false;
		let streamId: string | null = null;
		let needsCatchUp = false;
		const details = queryMatchers.exact(detailsQueryKey);
		const reads: QueryMatch = argoReads ? (query) => details(query) || argoReads(query) : details;
		const refresh = createLiveReadRefresh(queryClient, reads, onResourceRefresh);
		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "error") {
				realtimeWatchError = event.message;
				needsCatchUp = true;
				return;
			}
			if (event.type === "status") {
				if (event.status === "connected") {
					realtimeWatchError = null;
					if (needsCatchUp) {
						needsCatchUp = false;
						refresh.notify();
					}
				} else needsCatchUp = true;
				return;
			}
			if (event.type !== "resourceChanged") return;
			if (event.target.name && event.target.name !== resource.name) return;
			if (
				resource.namespace &&
				event.target.namespace &&
				event.target.namespace !== resource.namespace
			) {
				return;
			}
			refresh.notify();
		}, client);
		void startResourceWatch(
			client,
			resource.cluster,
			[watchKey],
			channel,
			source,
		)
			.then((id) => {
				if (cancelled) {
					void stopStream(client, id);
					return;
				}
				streamId = id;
			})
			.catch((cause: unknown) => {
				if (cancelled) return;
				realtimeWatchError = cause instanceof Error ? cause.message : String(cause);
			});
		return () => {
			cancelled = true;
			refresh.dispose();
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	});

	$effect(() => {
		const watch = eventWatch();
		if (!watch) return;
		const client = getClient();
		const { source, key: scope, read: { eventsQueryKey, eventWatch: target } } = watch;
		let cancelled = false;
		let retireAttempt: (() => void) | undefined;
		let retryTimer: ReturnType<typeof setTimeout> | undefined;
		const refresh = createLiveReadRefresh(queryClient, queryMatchers.prefix(eventsQueryKey));
		diagnosticLog("detail.events.watch", { key: scope });
		eventsWatchPhase = "opening";
		realtimeEventsWatchError = null;
		function receive(event: StreamMessage) {
			if (event.type === "error") {
				realtimeEventsWatchError = messageFromError(event.message);
				eventsWatchPhase = "recovering";
				return;
			}
			if (event.type === "status") {
				if (event.status === "connected") {
					realtimeEventsWatchError = null;
					const needsCatchUp = eventsWatchPhase !== "connected";
					eventsWatchPhase = "connected";
					if (needsCatchUp) refresh.notify();
				} else eventsWatchPhase = "recovering";
				return;
			}
			if (event.type === "stopped") {
				retireAttempt?.();
				eventsWatchPhase = "recovering";
				retryTimer = setTimeout(connect, RESOURCE_RECOVERY_INTERVAL);
				return;
			}
			if (event.type === "resourceEventsChanged") refresh.notify();
		}
		function connect() {
			retryTimer = undefined;
			let closed = false;
			let streamId: string | null = null;
			const channel = createStreamChannel((event: StreamMessage) => {
				if (!closed) receive(event);
			}, client);
			const close = () => {
				if (closed) return;
				closed = true;
				closeStreamChannel(channel);
				if (streamId) void stopStream(client, streamId).catch(() => {});
			};
			retireAttempt = close;
			void startResourceEventWatch(client, target.cluster, target.kind, target.name, target.namespace, channel, source)
				.then((id) => {
					if (closed) void stopStream(client, id).catch(() => {});
					else streamId = id;
				})
				.catch((cause: unknown) => {
					if (cancelled || closed) return;
					close();
					realtimeEventsWatchError = messageFromError(cause);
					eventsWatchPhase = "recovering";
					if (isTransientQueryError(cause)) retryTimer = setTimeout(connect, RESOURCE_RECOVERY_INTERVAL);
				});
		}
		connect();
		return () => {
			cancelled = true;
			if (retryTimer !== undefined) clearTimeout(retryTimer);
			refresh.dispose();
			retireAttempt?.();
		};
	});

	return {
		get resourceError() { return realtimeWatchError; },
		get eventsError() { return realtimeEventsWatchError; },
		get eventsConnected() { return eventsWatchPhase === "connected"; },
	};
}

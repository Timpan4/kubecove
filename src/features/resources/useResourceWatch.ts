import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import {
	closeStreamChannel,
	createStreamChannel,
	startResourceWatch,
	stopStream,
	type TauriClient,
} from "@/lib/tauri";
import type { StreamMessage, WatchResourceKey, WatchResourceTarget } from "@/lib/types";

interface WatchSubscription {
	keys: WatchResourceKey[];
	queryKeys: QueryKey[];
}

interface UseResourceWatchArgs {
	client: TauriClient;
	clusterContext: string;
	subscriptions: WatchSubscription[];
	enabled: boolean;
}

function watchKeySignature(key: WatchResourceKey): string {
	const kind = key.resourceKind;
	return [
		kind.kind,
		kind.apiVersion ?? "",
		kind.plural ?? "",
		key.namespace ?? "",
	].join(":");
}

function dedupeWatchKeys(keys: WatchResourceKey[]): WatchResourceKey[] {
	const deduped = new Map<string, WatchResourceKey>();
	for (const key of keys) {
		deduped.set(watchKeySignature(key), key);
	}
	return Array.from(deduped.values());
}

function watchKeyMatchesTarget(
	key: WatchResourceKey,
	target: WatchResourceTarget,
): boolean {
	if (key.resourceKind.kind !== target.kind) return false;
	if (!key.namespace) return true;
	return key.namespace === (target.namespace ?? undefined);
}

export function useResourceWatch({
	client,
	clusterContext,
	subscriptions,
	enabled,
}: UseResourceWatchArgs) {
	const queryClient = useQueryClient();
	const [status, setStatus] = useState("idle");
	const [message, setMessage] = useState("Realtime idle");
	const [error, setError] = useState<string | null>(null);
	const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const pendingInvalidationsRef = useRef<Map<string, QueryKey>>(new Map());
	const keys = useMemo(
		() => dedupeWatchKeys(subscriptions.flatMap((subscription) => subscription.keys)),
		[subscriptions],
	);
	const subscriptionSignature = useMemo(
		() => JSON.stringify(subscriptions),
		[subscriptions],
	);

	useEffect(() => {
		if (!enabled || keys.length === 0) {
			setStatus("idle");
			setMessage("Realtime idle");
			setError(null);
			return;
		}

		let cancelled = false;
		let streamId: string | null = null;
		setStatus("connecting");
		setMessage("Starting realtime watch");
		setError(null);

		const flushPendingInvalidations = () => {
			const matchedQueryKeys = Array.from(
				pendingInvalidationsRef.current.values(),
			);
			pendingInvalidationsRef.current.clear();
			for (const queryKey of matchedQueryKeys) {
				void queryClient.invalidateQueries({ queryKey });
			}
		};

		const invalidateSoon = (target: WatchResourceTarget) => {
			for (const subscription of subscriptions) {
				if (
					!subscription.keys.some((key) => watchKeyMatchesTarget(key, target))
				) {
					continue;
				}
				for (const queryKey of subscription.queryKeys) {
					pendingInvalidationsRef.current.set(JSON.stringify(queryKey), queryKey);
				}
			}
			if (debounceRef.current) clearTimeout(debounceRef.current);
			debounceRef.current = setTimeout(flushPendingInvalidations, 250);
		};

		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "started") {
				streamId = event.streamId;
				setMessage("Realtime watch starting");
				setError(null);
				return;
			}
			if (event.type === "status") {
				setStatus(event.status);
				setMessage(event.message);
				setError(null);
				return;
			}
			if (event.type === "resourceChanged") {
				setStatus("connected");
				setMessage(`Realtime ${event.action}`);
				setError(null);
				invalidateSoon(event.target);
				return;
			}
			if (event.type === "error") {
				setStatus("error");
				setError(event.message);
				setMessage("Realtime watch error");
				return;
			}
			if (event.type === "stopped") {
				setStatus("stopped");
				setMessage("Realtime stopped");
				setError(null);
			}
		});

		void startResourceWatch(client, clusterContext, keys, channel).then((id) => {
			if (cancelled) {
				void stopStream(client, id);
				return;
			}
			streamId = id;
		}).catch((err: unknown) => {
			if (cancelled) return;
			setStatus("error");
			setMessage("Realtime watch failed");
			setError(err instanceof Error ? err.message : String(err));
		});

		return () => {
			cancelled = true;
			if (debounceRef.current) clearTimeout(debounceRef.current);
			pendingInvalidationsRef.current.clear();
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	}, [
		client,
		clusterContext,
		enabled,
		keys,
		queryClient,
		subscriptionSignature,
		subscriptions,
	]);

	return { status, message, error };
}

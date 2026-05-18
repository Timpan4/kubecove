import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import {
	closeStreamChannel,
	createStreamChannel,
	startResourceWatch,
	stopStream,
	type TauriClient,
} from "@/lib/tauri";
import type { StreamMessage, WatchResourceKey } from "@/lib/types";

interface UseResourceWatchArgs {
	client: TauriClient;
	clusterContext: string;
	keys: WatchResourceKey[];
	queryKey: QueryKey;
	enabled: boolean;
}

export function useResourceWatch({
	client,
	clusterContext,
	keys,
	queryKey,
	enabled,
}: UseResourceWatchArgs) {
	const queryClient = useQueryClient();
	const [status, setStatus] = useState("idle");
	const [message, setMessage] = useState("Realtime idle");
	const [error, setError] = useState<string | null>(null);
	const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const keySignature = useMemo(() => JSON.stringify(keys), [keys]);

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

		const invalidateSoon = () => {
			if (debounceRef.current) clearTimeout(debounceRef.current);
			debounceRef.current = setTimeout(() => {
				void queryClient.invalidateQueries({ queryKey });
			}, 250);
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
				invalidateSoon();
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
			if (streamId) void stopStream(client, streamId);
			closeStreamChannel(channel);
		};
	}, [client, clusterContext, enabled, keySignature, queryClient, queryKey]);

	return { status, message, error };
}

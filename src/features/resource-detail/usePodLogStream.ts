import { useEffect, useRef, useState } from "react";
import {
	closeStreamChannel,
	createStreamChannel,
	startPodLogStream,
	stopStream,
	type TauriClient,
} from "@/lib/tauri";
import type { PodLogStreamRequest, StreamMessage } from "@/lib/types";

interface UsePodLogStreamArgs {
	client: TauriClient;
	request: PodLogStreamRequest | null;
	enabled: boolean;
}

const MAX_LOG_LINES = 1_000;

export function usePodLogStream({
	client,
	request,
	enabled,
}: UsePodLogStreamArgs) {
	const [lines, setLines] = useState<string[]>([]);
	const [status, setStatus] = useState("idle");
	const [message, setMessage] = useState("Log stream idle");
	const [error, setError] = useState<string | null>(null);
	const [version, setVersion] = useState(0);
	const streamIdRef = useRef<string | null>(null);
	const signature = request ? JSON.stringify(request) : "";

	useEffect(() => {
		setLines([]);
		setVersion(0);
	}, [signature]);

	useEffect(() => {
		if (!enabled || !request) {
			setStatus("idle");
			setMessage("Log stream idle");
			setError(null);
			return;
		}

		let cancelled = false;
		setStatus("connecting");
		setMessage("Starting log stream");
		setError(null);

		const channel = createStreamChannel((event: StreamMessage) => {
			if (cancelled) return;
			if (event.type === "started") {
				streamIdRef.current = event.streamId;
				return;
			}
			if (event.type === "status") {
				setStatus(event.status);
				setMessage(event.message);
				return;
			}
			if (event.type === "logLine") {
				setStatus("connected");
				setLines((current) => {
					const next = [...current, event.line];
					return next.length > MAX_LOG_LINES
						? next.slice(next.length - MAX_LOG_LINES)
						: next;
				});
				setVersion((current) => current + 1);
				return;
			}
			if (event.type === "error") {
				setStatus("error");
				setMessage("Log stream error");
				setError(event.message);
				return;
			}
			if (event.type === "stopped") {
				setStatus("stopped");
				setMessage("Log stream stopped");
			}
		});

		void startPodLogStream(client, request, channel).then((streamId) => {
			if (cancelled) {
				void stopStream(client, streamId);
				return;
			}
			streamIdRef.current = streamId;
		}).catch((err: unknown) => {
			if (cancelled) return;
			setStatus("error");
			setMessage("Log stream failed");
			setError(err instanceof Error ? err.message : String(err));
		});

		return () => {
			cancelled = true;
			if (streamIdRef.current) void stopStream(client, streamIdRef.current);
			closeStreamChannel(channel);
			streamIdRef.current = null;
		};
	}, [client, enabled, request, signature]);

	return {
		lines,
		version,
		status,
		message,
		error,
		clear: () => {
			setLines([]);
			setVersion(0);
		},
	};
}

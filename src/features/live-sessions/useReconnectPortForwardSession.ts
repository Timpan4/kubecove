import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
	startPortForward,
	stopPodPortForward,
	type TauriClient,
} from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import { portForwardSessionToRequest } from "./helpers";

interface UseReconnectPortForwardSessionOptions {
	client: TauriClient;
	onSuccess?: (session: PortForwardSessionSummary) => void;
	onError?: (error: unknown) => void;
}

export function useReconnectPortForwardSession({
	client,
	onSuccess,
	onError,
}: UseReconnectPortForwardSessionOptions) {
	const queryClient = useQueryClient();
	const [reconnectingId, setReconnectingId] = useState<string | null>(null);

	const reconnectSession = useCallback(
		async (session: PortForwardSessionSummary) => {
			setReconnectingId(session.id);
			try {
				await stopPodPortForward(client, session.id);
				await startPortForward(client, portForwardSessionToRequest(session));
				await queryClient.invalidateQueries({
					queryKey: queryKeys.portForwards(),
				});
				onSuccess?.(session);
			} catch (error) {
				onError?.(error);
			}
			setReconnectingId(null);
		},
		[client, onError, onSuccess, queryClient],
	);

	return { reconnectingId, reconnectSession };
}

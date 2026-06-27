import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { TauriClient } from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import { queryKeys } from "@/lib/queryKeys";
import { reconnectPortForwardSession } from "./saved-port-forward-actions";

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
				await reconnectPortForwardSession({ client, session });
				await queryClient.invalidateQueries({
					queryKey: queryKeys.portForwards(),
				});
			onSuccess?.(session);
		} catch (error) {
			setReconnectingId(null);
			onError?.(error);
			return;
		}
		setReconnectingId(null);
	},
		[client, onError, onSuccess, queryClient],
	);

	return { reconnectingId, reconnectSession };
}

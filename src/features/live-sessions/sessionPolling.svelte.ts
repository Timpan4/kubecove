import { useQueryClient, type QueryClient } from "@tanstack/svelte-query";
import { untrack } from "svelte";
import { queryKeys } from "@/lib/queryKeys";

export function trackLiveSessionUi(isOpen: () => boolean): void {
	const queryClient = useQueryClient();
	$effect(() => {
		if (!isOpen()) return;
		untrack(() => refreshLiveSessionQueries(queryClient));
	});
}

export function refreshLiveSessionQueries(queryClient: QueryClient): void {
	void queryClient.refetchQueries({ queryKey: queryKeys.portForwards(), type: "active" });
	void queryClient.refetchQueries({ queryKey: queryKeys.podExecSessions(), type: "active" });
}

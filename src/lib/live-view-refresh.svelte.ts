import { useQueryClient } from "@tanstack/svelte-query";
import {
	bindLiveView,
	IDLE_LIVE_VIEW,
	type LiveView,
	type LiveViewControls,
	type LiveViewState,
} from "./live-view-refresh";
import type { TauriClient } from "./tauri-runtime";

/** Binds the calling component to its live view. Null means nothing to watch or refresh. */
export function liveViewRefresh(client: TauriClient, view: () => LiveView | null): LiveViewControls {
	const queryClient = useQueryClient();
	let state = $state.raw<LiveViewState>(IDLE_LIVE_VIEW);
	let refreshBinding: (() => Promise<string[]>) | null = null;
	const key = $derived.by(() => {
		const current = view();
		return current ? JSON.stringify([current.clusterContext, current.kubeconfigSourceKey ?? "", current.keys]) : "";
	});
	// The watches stop when the view changes or the component unmounts.
	$effect(() => {
		const current = view();
		if (!current) {
			state = IDLE_LIVE_VIEW;
			return;
		}
		const binding = bindLiveView({ client, queryClient, view: current, onState: (next) => (state = next) });
		refreshBinding = binding.refresh;
		return () => {
			binding.stop();
			if (refreshBinding === binding.refresh) refreshBinding = null;
		};
	});
	return {
		get state() {
			return state;
		},
		get key() {
			return key;
		},
		refresh: async () => (await refreshBinding?.()) ?? [],
	};
}

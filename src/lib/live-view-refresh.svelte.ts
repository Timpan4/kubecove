import { useQueryClient } from "@tanstack/svelte-query";
import { onDestroy, untrack } from "svelte";
import {
	bindLiveView,
	IDLE_LIVE_VIEW,
	type LiveView,
	type LiveViewBinding,
	type LiveViewControls,
	type LiveViewState,
} from "./live-view-refresh";
import type { TauriClient } from "./tauri-runtime";

function ownerOf(view: LiveView): string {
	return JSON.stringify([view.clusterContext, view.kubeconfigSourceKey ?? ""]);
}

/** Binds the calling component to its live view. Null means nothing to watch or refresh. */
export function liveViewRefresh(client: TauriClient, view: () => LiveView | null): LiveViewControls {
	const queryClient = useQueryClient();
	let state = $state.raw<LiveViewState>(IDLE_LIVE_VIEW);
	let bound: { owner: string; binding: LiveViewBinding } | null = null;
	const key = $derived.by(() => {
		const current = view();
		return current ? JSON.stringify([current.clusterContext, current.kubeconfigSourceKey ?? "", current.keys]) : "";
	});
	function unbind() {
		bound?.binding.stop();
		bound = null;
	}
	onDestroy(unbind);
	// Only a new owner (cluster context or kubeconfig source) rebinds. Other view changes
	// update the binding in place, so watches whose keys stay in the view keep streaming.
	$effect(() => {
		const current = view();
		untrack(() => {
			if (!current) {
				unbind();
				state = IDLE_LIVE_VIEW;
			} else if (bound?.owner === ownerOf(current)) {
				bound.binding.update(current);
			} else {
				unbind();
				bound = {
					owner: ownerOf(current),
					binding: bindLiveView({ client, queryClient, view: current, onState: (next) => (state = next) }),
				};
			}
		});
	});
	return {
		get state() {
			return state;
		},
		get key() {
			return key;
		},
		refresh: async () => (await bound?.binding.refresh()) ?? [],
	};
}

import { useQueryClient } from "@tanstack/svelte-query";
import { onDestroy, untrack } from "svelte";
import {
	bindLiveView,
	combineLiveViewStates,
	type LiveView,
	type LiveViewBinding,
	type LiveViewControls,
	type LiveViewState,
} from "./live-view-refresh";
import type { TauriClient } from "./tauri-runtime";

function ownerOf(view: LiveView): string {
	return JSON.stringify([view.clusterContext, view.kubeconfigSourceKey ?? ""]);
}

type Slot = { clusterContext: string; state: LiveViewState | null };
type Bound = { slot: Slot; binding: LiveViewBinding };

/**
 * Binds the calling component to its live views, one per cluster context. Null
 * or an empty list means nothing to watch or refresh.
 */
export function liveViewRefresh(client: TauriClient, view: () => LiveView | readonly LiveView[] | null): LiveViewControls {
	const queryClient = useQueryClient();
	const views = () => {
		const current = view();
		return current === null ? [] : "reads" in current ? [current] : current;
	};
	let bound = new Map<string, Bound>();
	let states = $state.raw<{ clusterContext: string; state: LiveViewState }[]>([]);
	const state = $derived(combineLiveViewStates(states));
	const key = $derived(JSON.stringify(views().map((current) => [current.clusterContext, current.kubeconfigSourceKey ?? "", current.keys])));
	function publish() {
		states = [...bound.values()].flatMap(({ slot }) => (slot.state ? [{ clusterContext: slot.clusterContext, state: slot.state }] : []));
	}
	function unbind(entries: Iterable<Bound>) {
		for (const entry of entries) entry.binding.stop();
	}
	onDestroy(() => unbind(bound.values()));
	// Only a new owner (cluster context or kubeconfig source) rebinds. Other view changes
	// update the binding in place, so watches whose keys stay in the view keep streaming.
	$effect(() => {
		const current = views();
		untrack(() => {
			const next = new Map<string, Bound>();
			for (const item of current) {
				const owner = ownerOf(item);
				const kept = bound.get(owner);
				if (kept) {
					bound.delete(owner);
					kept.binding.update(item);
					next.set(owner, kept);
					continue;
				}
				const slot: Slot = { clusterContext: item.clusterContext, state: null };
				const binding = bindLiveView({ client, queryClient, view: item, onState: (nextState) => {
					slot.state = nextState;
					if (bound.get(owner)?.slot === slot) publish();
				} });
				next.set(owner, { slot, binding });
			}
			unbind(bound.values());
			bound = next;
			publish();
		});
	});
	return {
		get state() {
			return state;
		},
		get key() {
			return key;
		},
		refresh: async () => (await Promise.all([...bound.values()].map(({ binding }) => binding.refresh()))).flat(),
	};
}

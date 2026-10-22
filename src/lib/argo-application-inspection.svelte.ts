import { useQueryClient } from "@tanstack/svelte-query";
import { untrack } from "svelte";
import { fromStore } from "svelte/store";
import {
	createArgoApplicationInspection,
	type ArgoApplicationInspection,
	type ArgoInspectionTarget,
} from "./argo-application-inspection";
import { settingsStore } from "./settings-store";
import type { TauriClient } from "./tauri-runtime";

/** Argo Application inspection for the calling component; its reads stop when it unmounts. */
export function argoApplicationInspection(client: TauriClient, target: () => ArgoInspectionTarget) {
	const settings = fromStore(settingsStore);
	const input = () => ({ ...target(), settings: settings.current });
	const inspection = createArgoApplicationInspection(client, useQueryClient(), untrack(input));
	let current = $state.raw<ArgoApplicationInspection>(inspection.current);
	$effect.pre(() => inspection.update(input()));
	$effect(() => inspection.subscribe((next) => (current = next)));
	return {
		get current() {
			return current;
		},
		refresh: () => inspection.refresh(),
	};
}

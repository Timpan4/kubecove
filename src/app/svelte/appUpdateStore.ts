import { tick } from "svelte";
import { readable } from "svelte/store";
import {
	type AppUpdateState,
	useAppUpdateStore,
} from "@/features/app-updates/store";

// The readable's first value is captured at module load. A component that subscribes later must
// receive the current state, or it keeps a status from before the startup check finished.
export const appUpdateStore = readable<AppUpdateState>(
	useAppUpdateStore.getState(),
	(set) => {
		set(useAppUpdateStore.getState());
		return useAppUpdateStore.subscribe(set);
	},
);

export const appUpdateActions = {
	checkForUpdates: (options: Parameters<AppUpdateState["checkForUpdates"]>[0]) =>
		useAppUpdateStore.getState().checkForUpdates(options),
	installUpdate: () => useAppUpdateStore.getState().installUpdate(),
	relaunchApp: () => useAppUpdateStore.getState().relaunchApp(),
	dismissUpdate: (version: string) => useAppUpdateStore.getState().dismissUpdate(version),
};

export function scheduleAutomaticUpdateCheck(): () => void {
	let cancelled = false;
	let frame: number | undefined;
	void tick().then(() => {
		if (cancelled) return;
		frame = requestAnimationFrame(() => {
			if (!cancelled) void appUpdateActions.checkForUpdates({ manual: false });
		});
	});
	return () => {
		cancelled = true;
		if (frame !== undefined) cancelAnimationFrame(frame);
	};
}

import { readable } from "svelte/store";
import {
	type AppUpdateState,
	useAppUpdateStore,
} from "@/features/app-updates/store";

export const appUpdateStore = readable<AppUpdateState>(
	useAppUpdateStore.getState(),
	(set) => useAppUpdateStore.subscribe(set),
);

export const appUpdateActions = {
	checkForUpdates: (options: Parameters<AppUpdateState["checkForUpdates"]>[0]) =>
		useAppUpdateStore.getState().checkForUpdates(options),
	installUpdate: () => useAppUpdateStore.getState().installUpdate(),
	relaunchApp: () => useAppUpdateStore.getState().relaunchApp(),
	dismissUpdate: (version: string) => useAppUpdateStore.getState().dismissUpdate(version),
};

import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { flushPathStateWrite } from "@/lib/path-state";

import type { AppUpdate, AppUpdateApi, AppUpdateDownloadEvent } from "./types";

function mapDownloadEvent(event: DownloadEvent): AppUpdateDownloadEvent {
	return event;
}

export const tauriAppUpdateApi: AppUpdateApi = {
	check: async (): Promise<AppUpdate | null> => {
		const { check } = await import("@tauri-apps/plugin-updater");
		const update = await check();
		if (!update) return null;

		return {
			currentVersion: update.currentVersion,
			version: update.version,
			body: update.body,
			date: update.date,
			downloadAndInstall: (onEvent) =>
				update.downloadAndInstall((event) => onEvent?.(mapDownloadEvent(event))),
		};
	},
	relaunch: async () => {
		const { relaunch } = await import("@tauri-apps/plugin-process");
		flushPathStateWrite();
		return relaunch();
	},
};

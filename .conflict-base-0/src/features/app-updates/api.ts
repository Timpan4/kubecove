import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";

import type { AppUpdate, AppUpdateApi, AppUpdateDownloadEvent } from "./types";

function mapDownloadEvent(event: DownloadEvent): AppUpdateDownloadEvent {
	return event;
}

export const tauriAppUpdateApi: AppUpdateApi = {
	check: async (): Promise<AppUpdate | null> => {
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
	relaunch,
};

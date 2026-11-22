import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { flushPathStateWrite } from "@/lib/path-state";
import { exportTauriMigrationSnapshot } from "@/lib/tauri-migration";

import type { AppUpdate, AppUpdateApi, AppUpdateDownloadEvent } from "./types";

function mapDownloadEvent(event: DownloadEvent): AppUpdateDownloadEvent {
	return event;
}

// The feed has no GPUI build for .deb, .rpm or Intel Mac installs (ADR 0036).
function describeCheckError(error: unknown): Error {
	const message = error instanceof Error ? error.message : String(error);
	if (/was not found in the response `platforms` object|None of the fallback platforms/.test(message)) {
		return new Error(
			"The new KubeCove has no update for this install type. Download the AppImage, DMG or Windows installer from GitHub Releases.",
		);
	}
	return error instanceof Error ? error : new Error(message);
}

export const tauriAppUpdateApi: AppUpdateApi = {
	check: async (): Promise<AppUpdate | null> => {
		const { check } = await import("@tauri-apps/plugin-updater");
		const update = await check().catch((error: unknown) => {
			throw describeCheckError(error);
		});
		if (!update) return null;

		return {
			currentVersion: update.currentVersion,
			version: update.version,
			body: update.body,
			date: update.date,
			downloadAndInstall: async (onEvent) => {
				// The update replaces this app with KubeCove GPUI, which imports this snapshot once.
				try {
					await exportTauriMigrationSnapshot();
				} catch {
					console.warn("Could not save the migration snapshot.");
				}
				return update.downloadAndInstall((event) => onEvent?.(mapDownloadEvent(event)));
			},
		};
	},
	relaunch: async () => {
		const { relaunch } = await import("@tauri-apps/plugin-process");
		flushPathStateWrite();
		return relaunch();
	},
};

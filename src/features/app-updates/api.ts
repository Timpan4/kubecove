import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { flushPathStateWrite } from "@/lib/path-state";
import { exportTauriMigrationSnapshot } from "@/lib/tauri-migration";

import type { AppUpdate, AppUpdateApi, AppUpdateDownloadEvent } from "./types";

function mapDownloadEvent(event: DownloadEvent): AppUpdateDownloadEvent {
	return event;
}

// The feed has no GPUI build for .deb, .rpm or Intel Mac installs (ADR 0036).
const NO_TARGET = /was not found in the response `platforms` object|None of the fallback platforms/;
const NO_TARGET_MESSAGE =
	"The new KubeCove has no update for this install type. GitHub Releases lists the supported downloads.";

export const tauriAppUpdateApi: AppUpdateApi = {
	check: async (): Promise<AppUpdate | null> => {
		const { check } = await import("@tauri-apps/plugin-updater");
		let update: Awaited<ReturnType<typeof check>>;
		try {
			update = await check();
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			throw NO_TARGET.test(message) ? new Error(NO_TARGET_MESSAGE) : error;
		}
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

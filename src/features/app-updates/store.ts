import { createStore } from "zustand/vanilla";
import {
	createJSONStorage,
	persist,
	type StateStorage,
} from "zustand/middleware";

import { APP_VERSION } from "@/lib/app-version";
import { diagnosticLog } from "@/lib/diagnostics";
import {
	isAppUpdatesEnabled,
	setAppUpdatesEnabledForTests,
} from "@/lib/release-channel";
import { tauriAppUpdateApi } from "./api";
import type {
	AppUpdate,
	AppUpdateApi,
	AppUpdateCheckOptions,
	AppUpdateStatus,
} from "./types";

export interface AppUpdateState {
	status: AppUpdateStatus;
	currentVersion: string;
	availableVersion: string | null;
	releaseNotes: string | null;
	downloadProgress: number | null;
	downloadedBytes: number;
	totalBytes: number | null;
	lastCheckedAt: string | null;
	errorMessage: string | null;
	dismissedVersion: string | null;
	checkForUpdates: (options: AppUpdateCheckOptions) => Promise<void>;
	installUpdate: () => Promise<void>;
	relaunchApp: () => Promise<void>;
	dismissUpdate: (version: string) => void;
}

let updateApi: AppUpdateApi = tauriAppUpdateApi;
let pendingUpdate: AppUpdate | null = null;

const initialState = {
	status: "idle",
	currentVersion: APP_VERSION,
	availableVersion: null,
	releaseNotes: null,
	downloadProgress: null,
	downloadedBytes: 0,
	totalBytes: null,
	lastCheckedAt: null,
	errorMessage: null,
	dismissedVersion: null,
} satisfies Pick<
	AppUpdateState,
	| "status"
	| "currentVersion"
	| "availableVersion"
	| "releaseNotes"
	| "downloadProgress"
	| "downloadedBytes"
	| "totalBytes"
	| "lastCheckedAt"
	| "errorMessage"
	| "dismissedVersion"
>;

const memoryStorage = new Map<string, string>();

const fallbackStorage: StateStorage = {
	getItem: (name) => memoryStorage.get(name) ?? null,
	setItem: (name, value) => {
		memoryStorage.set(name, value);
	},
	removeItem: (name) => {
		memoryStorage.delete(name);
	},
};

function appUpdateStorage(): StateStorage {
	return globalThis.localStorage ?? fallbackStorage;
}

export const useAppUpdateStore = createStore<AppUpdateState>()(
	persist(
		(set, get) => ({
			...initialState,
			checkForUpdates: async ({ manual }) => {
				diagnosticLog("updates.check.start", { manual });
				if (!isAppUpdatesEnabled()) {
					pendingUpdate = null;
					set({
						status: "idle",
						availableVersion: null,
						releaseNotes: null,
						errorMessage: null,
						downloadProgress: null,
						downloadedBytes: 0,
						totalBytes: null,
					});
					diagnosticLog("updates.check.skipped", { manual });
					return;
				}

				set({
					status: "checking",
					errorMessage: null,
					downloadProgress: null,
					downloadedBytes: 0,
					totalBytes: null,
				});

				try {
					const update = await updateApi.check();
					const checkedAt = new Date().toISOString();

					if (!update) {
						pendingUpdate = null;
						set({
							status: "upToDate",
							availableVersion: null,
							releaseNotes: null,
							lastCheckedAt: checkedAt,
							errorMessage: null,
						});
						diagnosticLog("updates.check.done", { available: false });
						return;
					}

					pendingUpdate = update;
					set({
						status: "available",
						currentVersion: update.currentVersion || APP_VERSION,
						availableVersion: update.version,
						releaseNotes: update.body ?? null,
						lastCheckedAt: checkedAt,
						errorMessage: null,
					});
					diagnosticLog("updates.check.done", {
						available: true,
						version: update.version,
					});
				} catch (error) {
					const message = error instanceof Error ? error.message : String(error);
					pendingUpdate = null;
					set({
						status: "error",
						lastCheckedAt: new Date().toISOString(),
						availableVersion: null,
						releaseNotes: null,
						errorMessage: message,
					});
					diagnosticLog("updates.check.error", { error: message });
				}
			},
			installUpdate: async () => {
				const update = pendingUpdate;
				if (!update) {
					set({
						status: "error",
						errorMessage: "No downloaded update is available to install.",
					});
					return;
				}

				set({
					status: "downloading",
					errorMessage: null,
					downloadProgress: 0,
					downloadedBytes: 0,
					totalBytes: null,
				});

				try {
					await update.downloadAndInstall((event) => {
						if (event.event === "Started") {
							const totalBytes = event.data.contentLength ?? null;
							set({
								totalBytes,
								downloadedBytes: 0,
								downloadProgress: totalBytes ? 0 : null,
							});
							return;
						}

						if (event.event === "Progress") {
							const current = get();
							const downloadedBytes =
								current.downloadedBytes + event.data.chunkLength;
							set({
								downloadedBytes,
								downloadProgress: current.totalBytes
									? Math.min(
											100,
											Math.round((downloadedBytes / current.totalBytes) * 100),
										)
									: null,
							});
							return;
						}

						set({ downloadProgress: 100 });
					});

					set({
						status: "installed",
						downloadProgress: 100,
						errorMessage: null,
					});
					diagnosticLog("updates.install.done");
				} catch (error) {
					const message = error instanceof Error ? error.message : String(error);
					set({
						status: "error",
						errorMessage: message,
					});
					diagnosticLog("updates.install.error", { error: message });
				}
			},
				relaunchApp: async () => {
					try {
						await updateApi.relaunch();
					} catch (error) {
						const message = error instanceof Error ? error.message : String(error);
						set({
							status: "error",
							errorMessage: message,
					});
				}
			},
			dismissUpdate: (version: string) => set({ dismissedVersion: version }),
		}),
		{
			name: "kubecove-app-updates",
			storage: createJSONStorage(appUpdateStorage),
			partialize: (state) => ({
				dismissedVersion: state.dismissedVersion,
				lastCheckedAt: state.lastCheckedAt,
			}),
		},
	),
);

export function setAppUpdateApiForTests(api: AppUpdateApi): void {
	updateApi = api;
}

export function resetAppUpdateStateForTests(): void {
	updateApi = tauriAppUpdateApi;
	pendingUpdate = null;
	setAppUpdatesEnabledForTests(true);
	useAppUpdateStore.setState(initialState);
}

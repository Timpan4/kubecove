import type { AppUpdateApi } from "./types";

export const browserAppUpdateApi: AppUpdateApi = {
	check: async () => null,
	relaunch: async () => {
		throw new Error("Installed app updates are unavailable in the browser.");
	},
};

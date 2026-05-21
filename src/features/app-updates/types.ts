export type AppUpdateStatus =
	| "idle"
	| "checking"
	| "available"
	| "downloading"
	| "installed"
	| "upToDate"
	| "error";

export type AppUpdateCheckOptions = {
	manual: boolean;
};

export type AppUpdateDownloadEvent =
	| {
			event: "Started";
			data: {
				contentLength?: number;
			};
	  }
	| {
			event: "Progress";
			data: {
				chunkLength: number;
			};
	  }
	| {
			event: "Finished";
	  };

export interface AppUpdate {
	currentVersion: string;
	version: string;
	body?: string;
	date?: string;
	downloadAndInstall: (
		onEvent?: (event: AppUpdateDownloadEvent) => void,
	) => Promise<void>;
}

export interface AppUpdateApi {
	check: () => Promise<AppUpdate | null>;
	relaunch: () => Promise<void>;
}

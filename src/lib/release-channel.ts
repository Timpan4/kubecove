export type AppReleaseChannel = "dev" | "stable";

const rawReleaseChannel = import.meta.env.VITE_KUBECOVE_RELEASE_CHANNEL;

export const APP_RELEASE_CHANNEL: AppReleaseChannel =
	rawReleaseChannel === "stable" ? "stable" : "dev";

let appUpdatesEnabledForTests: boolean | null = null;

export function isAppUpdatesEnabled(): boolean {
	return appUpdatesEnabledForTests ?? APP_RELEASE_CHANNEL === "stable";
}

export function setAppUpdatesEnabledForTests(enabled: boolean | null): void {
	appUpdatesEnabledForTests = enabled;
}

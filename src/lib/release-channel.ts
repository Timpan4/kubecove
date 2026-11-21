import { RELEASE_CHANNEL } from "./build-env";

export type AppReleaseChannel = "dev" | "stable";

export const APP_RELEASE_CHANNEL: AppReleaseChannel =
	RELEASE_CHANNEL;

let appUpdatesEnabledForTests = false;

export function isAppUpdatesEnabled(): boolean {
	return appUpdatesEnabledForTests;
}

export function setAppUpdatesEnabledForTests(enabled: boolean): void {
	appUpdatesEnabledForTests = enabled;
}

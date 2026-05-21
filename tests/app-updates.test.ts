import { describe, expect, test } from "bun:test";

import {
	resetAppUpdateStateForTests,
	setAppUpdateApiForTests,
	useAppUpdateStore,
} from "../src/features/app-updates/store";
import type { AppUpdate, AppUpdateApi } from "../src/features/app-updates/types";
import { setAppUpdatesEnabledForTests } from "../src/lib/release-channel";

function mockApi(update: AppUpdate | null): AppUpdateApi {
	return {
		check: async () => update,
		relaunch: async () => undefined,
	};
}

describe("app update store", () => {
	test("launch check transitions to available when an update exists", async () => {
		resetAppUpdateStateForTests();
		setAppUpdateApiForTests(
			mockApi({
				currentVersion: "0.2.0",
				version: "0.3.0",
				body: "Release notes",
				downloadAndInstall: async () => undefined,
			}),
		);

		await useAppUpdateStore.getState().checkForUpdates({ manual: false });

		expect(useAppUpdateStore.getState().status).toBe("available");
		expect(useAppUpdateStore.getState().availableVersion).toBe("0.3.0");
		expect(useAppUpdateStore.getState().releaseNotes).toBe("Release notes");
	});

	test("manual check transitions to up to date when no update exists", async () => {
		resetAppUpdateStateForTests();
		setAppUpdateApiForTests(mockApi(null));

		await useAppUpdateStore.getState().checkForUpdates({ manual: true });

		expect(useAppUpdateStore.getState().status).toBe("upToDate");
		expect(useAppUpdateStore.getState().availableVersion).toBeNull();
		expect(useAppUpdateStore.getState().lastCheckedAt).not.toBeNull();
	});

	test("download progress updates from updater events", async () => {
		resetAppUpdateStateForTests();
		setAppUpdateApiForTests(
			mockApi({
				currentVersion: "0.2.0",
				version: "0.3.0",
				downloadAndInstall: async (onEvent) => {
					onEvent?.({ event: "Started", data: { contentLength: 100 } });
					onEvent?.({ event: "Progress", data: { chunkLength: 40 } });
					expect(useAppUpdateStore.getState().downloadProgress).toBe(40);
					onEvent?.({ event: "Progress", data: { chunkLength: 60 } });
					onEvent?.({ event: "Finished" });
				},
			}),
		);

		await useAppUpdateStore.getState().checkForUpdates({ manual: true });
		await useAppUpdateStore.getState().installUpdate();

		expect(useAppUpdateStore.getState().status).toBe("installed");
		expect(useAppUpdateStore.getState().downloadProgress).toBe(100);
	});

	test("dismissed version is retained while the update remains available", async () => {
		resetAppUpdateStateForTests();
		setAppUpdateApiForTests(
			mockApi({
				currentVersion: "0.2.0",
				version: "0.3.0",
				downloadAndInstall: async () => undefined,
			}),
		);

		await useAppUpdateStore.getState().checkForUpdates({ manual: false });
		useAppUpdateStore.getState().dismissUpdate("0.3.0");

		expect(useAppUpdateStore.getState().status).toBe("available");
		expect(useAppUpdateStore.getState().dismissedVersion).toBe("0.3.0");
	});

	test("check failures become visible error state", async () => {
		resetAppUpdateStateForTests();
		setAppUpdateApiForTests({
			check: async () => {
				throw new Error("network unavailable");
			},
			relaunch: async () => undefined,
		});

		await useAppUpdateStore.getState().checkForUpdates({ manual: false });

		expect(useAppUpdateStore.getState().status).toBe("error");
		expect(useAppUpdateStore.getState().errorMessage).toBe(
			"network unavailable",
		);
	});

	test("disabled update channel skips manual checks", async () => {
		resetAppUpdateStateForTests();
		setAppUpdatesEnabledForTests(false);
		let checks = 0;
		setAppUpdateApiForTests({
			check: async () => {
				checks += 1;
				throw new Error("should not check");
			},
			relaunch: async () => undefined,
		});

		await useAppUpdateStore.getState().checkForUpdates({ manual: true });

		expect(checks).toBe(0);
		expect(useAppUpdateStore.getState().status).toBe("idle");
		expect(useAppUpdateStore.getState().lastCheckedAt).toBeNull();
		expect(useAppUpdateStore.getState().errorMessage).toBeNull();
		setAppUpdatesEnabledForTests(true);
	});
});

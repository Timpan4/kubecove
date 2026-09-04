import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import {
	resetAppUpdateStateForTests,
	setAppUpdateApiForTests,
	useAppUpdateStore,
} from "../src/features/app-updates/store";
import type { AppUpdate, AppUpdateApi } from "../src/features/app-updates/types";
import { setAppUpdatesEnabledForTests } from "../src/lib/release-channel";
import { tick } from "svelte";
import { scheduleAutomaticUpdateCheck } from "../src/app/svelte/appUpdateStore";

function mockApi(update: AppUpdate | null): AppUpdateApi {
	return {
		check: async () => update,
		relaunch: async () => undefined,
	};
}

describe("app update store", () => {
	test("automatic checks wait for the DOM flush and animation frame", async () => {
		resetAppUpdateStateForTests();
		let checks = 0;
		setAppUpdateApiForTests({ check: async () => { checks += 1; return null; }, relaunch: async () => {} });
		const originalRequest = globalThis.requestAnimationFrame;
		const originalCancel = globalThis.cancelAnimationFrame;
		const frames: FrameRequestCallback[] = [];
		globalThis.requestAnimationFrame = (callback) => frames.push(callback) - 1;
		globalThis.cancelAnimationFrame = () => {};
		const cancel = scheduleAutomaticUpdateCheck();
		try {
			expect(frames).toHaveLength(0);
			expect(checks).toBe(0);
			await tick();
			await Promise.resolve();
			expect(frames).toHaveLength(1);
			expect(checks).toBe(0);
			frames[0]?.(0);
			expect(checks).toBe(1);
		} finally {
			cancel();
			globalThis.requestAnimationFrame = originalRequest;
			globalThis.cancelAnimationFrame = originalCancel;
			resetAppUpdateStateForTests();
		}
	});

	for (const afterDomFlush of [false, true]) {
		test(`unmount cancels automatic checks, after DOM flush: ${afterDomFlush}`, async () => {
			resetAppUpdateStateForTests();
			let checks = 0;
			setAppUpdateApiForTests({ check: async () => { checks += 1; return null; }, relaunch: async () => {} });
			const originalRequest = globalThis.requestAnimationFrame;
			const originalCancel = globalThis.cancelAnimationFrame;
			const frames: FrameRequestCallback[] = [];
			const cancelled: number[] = [];
			globalThis.requestAnimationFrame = (callback) => frames.push(callback) - 1;
			globalThis.cancelAnimationFrame = (frame) => { cancelled.push(frame); };
			try {
				const cancel = scheduleAutomaticUpdateCheck();
				if (afterDomFlush) { await tick(); await Promise.resolve(); }
				cancel();
				await tick();
				await Promise.resolve();
				for (const frame of frames) frame(0);
				expect(checks).toBe(0);
				expect(cancelled).toEqual(afterDomFlush ? [0] : []);
				expect(frames).toHaveLength(afterDomFlush ? 1 : 0);
			} finally {
				globalThis.requestAnimationFrame = originalRequest;
				globalThis.cancelAnimationFrame = originalCancel;
				resetAppUpdateStateForTests();
			}
		});
	}

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

	test("Svelte runtime keeps launch checks and disabled-channel chrome parity", () => {
		const appSource = readFileSync("src/app/svelte/App.svelte", "utf8");
		const settingsSource = readFileSync(
			"src/app/svelte/UpdatesSettings.svelte",
			"utf8",
		);
		const buttonSource = readFileSync(
			"src/app/svelte/UpdateStatusButton.svelte",
			"utf8",
		);
		const bridgeSource = readFileSync(
			"src/app/svelte/appUpdateStore.ts",
			"utf8",
		);

		expect(appSource).toContain("isAppUpdatesEnabled()");
		expect(appSource).toContain(
			"return scheduleAutomaticUpdateCheck()",
		);
		expect(buttonSource).toContain("isAppUpdatesEnabled()");
		expect(buttonSource).toContain("{#if updatesEnabled}");
		expect(buttonSource).toContain("appUpdateStore");
		expect(buttonSource).toContain("$appUpdateStore");
		expect(settingsSource).toContain("appUpdateActions.checkForUpdates");
		expect(settingsSource).toContain("appUpdateActions.installUpdate");
		expect(settingsSource).toContain("appUpdateActions.relaunchApp");
		expect(settingsSource).toContain("$appUpdateStore");
		expect(bridgeSource).toContain("readable<AppUpdateState>");
		expect(bridgeSource).toContain("useAppUpdateStore.subscribe(set)");
		expect(bridgeSource).toContain("checkForUpdates");
		expect(bridgeSource).toContain("dismissUpdate");
		expect(appSource).not.toContain("useAppUpdateStore");
		expect(buttonSource).not.toContain("useAppUpdateStore");
		expect(settingsSource).not.toContain("useAppUpdateStore");
	});

	test("Svelte update chrome keeps release workflow parity", () => {
		const buttonSource = readFileSync(
			"src/app/svelte/UpdateStatusButton.svelte",
			"utf8",
		);

		expect(buttonSource).toContain("let panelOpen = $state(false)");
		expect(buttonSource).toContain("let autoOpenedVersion = $state<string | null>(null)");
		expect(buttonSource).toContain("update.dismissedVersion !== update.availableVersion");
		expect(buttonSource).toContain("update.releaseNotes");
		expect(buttonSource).toContain("dismissAvailableUpdate");
		expect(buttonSource).toContain("update.dismissUpdate(update.availableVersion)");
		expect(buttonSource).toContain("update.installUpdate()");
		expect(buttonSource).toContain("update.relaunchApp()");
		expect(buttonSource).toContain("update.checkForUpdates({ manual: true })");
		expect(buttonSource).toContain("update.downloadProgress ?? 12");
		expect(buttonSource).toContain('role="dialog"');
		expect(buttonSource).toContain("<Popover bind:open={panelOpen}");
		expect(buttonSource).toContain("onOpenChange={handlePanelOpenChange}");
		expect(buttonSource).toContain("if (!open && update.availableVersion !== null) autoOpenedVersion = update.availableVersion");
		expect(buttonSource).toContain("<PopoverContent");
		expect(buttonSource).toContain("buttonClass({");
	});
});

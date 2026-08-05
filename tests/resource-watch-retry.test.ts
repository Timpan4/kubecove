import { describe, expect, test } from "bun:test";
import { startResourceWatchWithRetry } from "../src/lib/tauri-streams";
import type { TauriClient } from "../src/lib/tauri-runtime";

const channel = {} as Parameters<typeof startResourceWatchWithRetry>[3];
const keys = [{
	resourceKind: {
		kind: "Application",
		apiVersion: "argoproj.io/v1alpha1",
		plural: "applications",
		namespaced: true,
	},
}];

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("resource watch retry", () => {
	test("retries failed startup once without overlapping streams", async () => {
		let starts = 0;
		const client: TauriClient = {
			invoke: async <T>(command: string) => {
				if (command === "start_resource_watch") {
					starts += 1;
					if (starts === 1) throw new Error("startup failed");
					return "stream-1" as T;
				}
				return true as T;
			},
		};

		const dispose = startResourceWatchWithRetry(client, "cluster", keys, channel);
		await wait(300);
		expect(starts).toBe(2);
		dispose();
	});

	test("stops retry after cleanup", async () => {
		let starts = 0;
		const client: TauriClient = {
			invoke: async <T>(command: string) => {
				if (command === "start_resource_watch") {
					starts += 1;
					throw new Error("startup failed");
				}
				return true as T;
			},
		};

		const dispose = startResourceWatchWithRetry(client, "cluster", keys, channel);
		dispose();
		await wait(300);
		expect(starts).toBe(1);
	});
});

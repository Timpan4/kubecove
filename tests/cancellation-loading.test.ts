import { describe, expect, test } from "bun:test";
import {
	beginForegroundLoad,
	getForegroundLoadingSnapshot,
	withForegroundLoad,
} from "../src/lib/foreground-loading";

describe("foreground loading", () => {
	test("tracks foreground work until completion", async () => {
		expect(getForegroundLoadingSnapshot()).toBe(0);

		const done = beginForegroundLoad("resources");
		expect(getForegroundLoadingSnapshot()).toBe(1);
		done();
		expect(getForegroundLoadingSnapshot()).toBe(0);

		const value = await withForegroundLoad("details", async () => {
			expect(getForegroundLoadingSnapshot()).toBe(1);
			return "ok";
		});

		expect(value).toBe("ok");
		expect(getForegroundLoadingSnapshot()).toBe(0);
	});
});

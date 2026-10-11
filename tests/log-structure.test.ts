import { describe, expect, test } from "bun:test";
import {
	highlightSegments,
	parseLogStructure,
} from "../src/features/resource-detail/log-structure";

// Failure modes this parser must not hit:
// 1. Misses the level in zerolog/cloudflared, logfmt, JSON, or klog output.
// 2. Detects a level from a word in the middle of prose.
// 3. Splits quoted logfmt values on spaces, escaped quotes, or `=` inside URLs.
// 4. Turns prose that merely contains `=` into fields and drops the rest of the text.
// 5. Loses the message when a line only has fields (no prefix text).
// 6. Throws or drops text on malformed JSON.
// 7. Keeps the app's own timestamp prefix, duplicating the time gutter.
// 8. Highlights the wrong ranges or loses text around matches.

describe("parseLogStructure", () => {
	test("reads cloudflared/zerolog level tokens and strips the app timestamp", () => {
		expect(
			parseLogStructure(
				'2026-10-11T17:14:19Z ERR failed to serve incoming request error="Failed to proxy HTTP: EOF"',
			),
		).toEqual({
			level: "error",
			headline: "failed to serve incoming request",
			error: "Failed to proxy HTTP: EOF",
			fields: [],
		});
	});

	test("uses the error field as headline when a line only has fields", () => {
		expect(
			parseLogStructure(
				'2026-10-11T17:14:19Z ERR  error="Unable to reach the origin" connIndex=2 originService=https://127.0.0.1:8080/a?b=c',
			),
		).toEqual({
			level: "error",
			headline: "Unable to reach the origin",
			fields: [
				{ key: "connIndex", value: "2" },
				{ key: "originService", value: "https://127.0.0.1:8080/a?b=c" },
			],
		});
	});

	test("reads logfmt level and msg keys and drops time keys", () => {
		expect(
			parseLogStructure('time="2026-05-18T09:01:35Z" level=warning msg="disk \\"low\\"" free=10Gi'),
		).toEqual({
			level: "warn",
			headline: 'disk "low"',
			fields: [{ key: "free", value: "10Gi" }],
		});
	});

	test("reads JSON logs and stringifies nested values", () => {
		expect(
			parseLogStructure(
				'{"level":"ERROR","ts":1,"msg":"sync failed","err":"timeout","attempt":3,"ctx":{"ns":"a"}}',
			),
		).toEqual({
			level: "error",
			headline: "sync failed",
			error: "timeout",
			fields: [
				{ key: "attempt", value: "3" },
				{ key: "ctx", value: '{"ns":"a"}' },
			],
		});
	});

	test("keeps malformed JSON as plain text", () => {
		expect(parseLogStructure('{"level":"info"')).toEqual({
			headline: '{"level":"info"',
			fields: [],
		});
	});

	test("reads klog prefixes and quoted structured messages", () => {
		expect(
			parseLogStructure('W1011 17:14:19.936123       1 reflector.go:561] "Watch closed" reason="EOF"'),
		).toEqual({
			level: "warn",
			headline: "Watch closed",
			fields: [{ key: "reason", value: "EOF" }],
		});
		expect(parseLogStructure("E1011 17:14:19.936123 1 main.go:9] boom")).toMatchObject({
			level: "error",
			headline: "boom",
		});
	});

	test("does not treat level words inside prose as a level", () => {
		expect(parseLogStructure("request finished with INFO header")).toEqual({
			headline: "request finished with INFO header",
			fields: [],
		});
	});

	test("keeps prose with stray equals signs intact", () => {
		expect(parseLogStructure("INFO if a==b then retry=3 times")).toEqual({
			level: "info",
			headline: "if a==b then retry=3 times",
			fields: [],
		});
		expect(parseLogStructure("total = 4")).toEqual({ headline: "total = 4", fields: [] });
	});

	test("maps level aliases case-insensitively", () => {
		expect(parseLogStructure("[warn] slow").level).toBe("warn");
		expect(parseLogStructure("error: bad").level).toBe("error");
		expect(parseLogStructure("FATAL bad").level).toBe("error");
		expect(parseLogStructure("DEBUG x").level).toBe("debug");
		expect(parseLogStructure("TRACE x").level).toBe("debug");
		expect(parseLogStructure("level=notice msg=x").level).toBe("info");
	});

	test("leaves unleveled plain text unchanged", () => {
		expect(parseLogStructure("plain message")).toEqual({ headline: "plain message", fields: [] });
		expect(parseLogStructure("")).toEqual({ headline: "", fields: [] });
	});
});

describe("highlightSegments", () => {
	test("marks case-insensitive matches and keeps all text", () => {
		expect(highlightSegments("Origin origin", "origin")).toEqual([
			{ text: "Origin", match: true },
			{ text: " ", match: false },
			{ text: "origin", match: true },
		]);
	});

	test("returns the whole text when there is no term or match", () => {
		expect(highlightSegments("abc", "")).toEqual([{ text: "abc", match: false }]);
		expect(highlightSegments("abc", "z")).toEqual([{ text: "abc", match: false }]);
	});
});

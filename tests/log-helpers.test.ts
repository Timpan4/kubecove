import { describe, expect, test } from "bun:test";
import {
	formatExactTimeOnly,
	formatExactTimestamp,
} from "../src/components/timestamp-format";
import {
	appendParsedLogLine,
	latestTimestampedLogLine,
	logLineSearchText,
	MAX_RETAINED_LOG_LINES,
	orderedLogLines,
	parseLogLine,
	type ParsedLogLine,
} from "../src/features/resource-detail/log-helpers";

describe("log presentation helpers", () => {
	test("splits Kubernetes log timestamps from the message", () => {
		expect(
			parseLogLine(
				'2026-05-18T09:01:35.103840719Z time="2026-05-18T09:01:35Z" level=info msg="ok"',
			),
		).toEqual({
			index: 0,
			message: 'time="2026-05-18T09:01:35Z" level=info msg="ok"',
			raw: '2026-05-18T09:01:35.103840719Z time="2026-05-18T09:01:35Z" level=info msg="ok"',
			timestamp: "2026-05-18T09:01:35.103840719Z",
		});
	});

	test("uses embedded time fields when no leading timestamp exists", () => {
		expect(parseLogLine('level=info time="2026-05-18T09:01:35Z" msg="ok"')).toMatchObject({
			message: 'level=info time="2026-05-18T09:01:35Z" msg="ok"',
			timestamp: "2026-05-18T09:01:35Z",
		});
	});

	test("preserves raw untimestamped log lines", () => {
		expect(parseLogLine("plain message", 4)).toEqual({
			index: 4,
			message: "plain message",
			raw: "plain message",
			timestamp: undefined,
		});
	});

	test("preserves aggregate log source metadata for ordering and filtering", () => {
		const lines = orderedLogLines(
			[
				{
					line: "2026-05-18T09:01:35Z ready",
					source: { podName: "api-7f8d", container: "web" },
				},
				{
					line: "plain worker message",
					source: { podName: "api-9c2a", container: "worker" },
				},
			],
			false,
		);

		expect(lines[0]).toMatchObject({
			message: "ready",
			source: { podName: "api-7f8d", container: "web" },
			timestamp: "2026-05-18T09:01:35Z",
		});
		expect(logLineSearchText(lines[1])).toContain("api-9c2a");
		expect(logLineSearchText(lines[1])).toContain("worker");
		expect(logLineSearchText(lines[1])).toContain("plain worker message");
	});

	test("can show newest log lines first without mutating the original array", () => {
		const lines = ["first", "second", "third"];

		expect(orderedLogLines(lines, true).map((line) => line.message)).toEqual([
			"third",
			"second",
			"first",
		]);
		expect(lines).toEqual(["first", "second", "third"]);
	});

	test("finds latest timestamped log line by time and index", () => {
		const lines = [
			parseLogLine("2026-05-18T09:01:35Z first", 0),
			parseLogLine("2026-05-18T09:01:35Z second", 1),
			parseLogLine('level=info time="not-a-time" msg="bad"', 2),
		];

		expect(latestTimestampedLogLine(lines)).toMatchObject({
			index: 1,
			message: "second",
			timestamp: "2026-05-18T09:01:35Z",
		});
	});

	test("retains the newest parsed log lines without reparsing the buffer", () => {
		const lines: ParsedLogLine[] = [];
		for (let index = 0; index <= MAX_RETAINED_LOG_LINES; index += 1) {
			appendParsedLogLine(lines, `line ${index}`, index);
		}

		expect(lines).toHaveLength(MAX_RETAINED_LOG_LINES);
		expect(lines[0]?.index).toBe(1);
		expect(lines.at(-1)?.index).toBe(MAX_RETAINED_LOG_LINES);
	});

	test("formats log timestamps through the shared timestamp formatter", () => {
		expect(
			formatExactTimestamp(
				"2026-05-18T09:01:35.103840719Z",
				"utc",
				"millisecond",
			),
		).toBe("2026-05-18 09:01:35.103 UTC");
		expect(
			formatExactTimestamp("2026-05-18T09:01:35Z", "utc", "second"),
		).toBe("2026-05-18 09:01:35 UTC");
		expect(formatExactTimestamp("2026-05-18T09:01:35Z", "utc")).toBe(
			"2026-05-18 09:01 UTC",
		);
	});

	test("keeps full log timestamps out of the inline gutter", () => {
		const timestamp = "2026-05-18T09:01:35.103840719Z";
		expect(formatExactTimeOnly(timestamp, "utc")).toBe("09:01:35.103 UTC");
		expect(formatExactTimestamp(timestamp, "utc", "millisecond")).toBe(
			"2026-05-18 09:01:35.103 UTC",
		);
	});
});

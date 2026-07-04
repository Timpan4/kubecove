import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	formatExactTimeOnly,
	formatExactTimestamp,
} from "../src/components/timestamp-format";
import {
	latestTimestampedLogLine,
	orderedLogLines,
	parseLogLine,
} from "../src/features/resource-detail/log-helpers";

function extractFunctionSource(source: string, name: string): string | undefined {
	const start = source.indexOf(`function ${name}`);
	if (start < 0) return undefined;
	const bodyStart = source.indexOf("{", start);
	if (bodyStart < 0) return undefined;

	let depth = 0;
	for (let index = bodyStart; index < source.length; index += 1) {
		const character = source[index];
		if (character === "{") depth += 1;
		if (character === "}") {
			depth -= 1;
			if (depth === 0) return source.slice(start, index + 1);
		}
	}
	return undefined;
}

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

		const detailSource = readFileSync(
			"src/features/resource-detail/ResourceDetailPanel.svelte",
			"utf8",
		);
		const logTimeFunction = extractFunctionSource(detailSource, "formatLogTime");
		const logsSource = readFileSync(
			"src/features/resource-detail/LogsTab.svelte",
			"utf8",
		);

		expect(logTimeFunction).toContain(
			"return formatExactTimeOnly(timestamp, timestampTimezone)",
		);
		expect(logTimeFunction).not.toContain("formatExactTimestamp");
		expect(logsSource).toContain("title={formatFullTimestamp(line.timestamp)}");
		expect(logsSource).toContain("{formatLogTime(line.timestamp)}");
		expect(logsSource).not.toMatch(
			/>\s*\{formatFullTimestamp\(line\.timestamp\)\}\s*<\/time>/,
		);
	});

	test("Svelte logs use selected non-init containers before streaming", () => {
		const source = [
			readFileSync("src/features/resource-detail/ResourceDetailPanel.svelte", "utf8"),
			readFileSync("src/features/resource-detail/LogsTab.svelte", "utf8"),
		].join("\n");

		expect(source).toContain(
			'containerRows.filter((container) => container.type !== "init")',
		);
		expect(source).toContain("isPod && selectedContainer");
		expect(source).toContain("container: selectedContainer");
		expect(source).toContain("No containers found");
	});
});

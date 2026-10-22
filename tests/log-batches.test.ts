import { expect, test } from "bun:test";
import {
	appendParsedLogBatch,
	MAX_RETAINED_LOG_LINES,
	parseLogLine,
} from "../src/features/resource-detail/log-helpers";

// Failure modes: mutate retained history, trim the wrong end, reuse indices, lose source/empty lines.
test("log flush retains the newest lines in arrival order without mutating history", () => {
	const previous = [parseLogLine("old", 0)];
	const input = Array.from(
		{ length: MAX_RETAINED_LOG_LINES + 1 },
		(_, index) => ({
			line: index === MAX_RETAINED_LOG_LINES ? "" : String(index),
			source: { podName: "api", container: "app" },
		}),
	);
	const result = appendParsedLogBatch(previous, input, 1);
	expect(previous.map((line) => line.raw)).toEqual(["old"]);
	expect(result).toHaveLength(MAX_RETAINED_LOG_LINES);
	expect(result[0]?.index).toBe(2);
	expect(result.at(-1)?.index).toBe(input.length);
	expect(result.at(-1)?.raw).toBe("");
	expect(result.at(-1)?.source).toEqual(input[0]?.source);
});

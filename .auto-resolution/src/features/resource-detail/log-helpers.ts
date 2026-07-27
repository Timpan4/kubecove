import type { LogLineSource } from "@/lib/types";

export interface LogLineEntry {
	line: string;
	source?: LogLineSource;
}

export type LogLineInput = string | LogLineEntry;

export interface ParsedLogLine {
	index: number;
	message: string;
	raw: string;
	source?: LogLineSource;
	timestamp?: string;
}

const LEADING_TIMESTAMP_RE =
	/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(.*)$/;
const EMBEDDED_TIME_RE = /\btime=(?:"([^"]+)"|([^\s]+))/;

function entryFromLogLine(input: LogLineInput): LogLineEntry {
	return typeof input === "string" ? { line: input } : input;
}

export function parseLogLine(input: LogLineInput, index = 0): ParsedLogLine {
	const entry = entryFromLogLine(input);
	const line = entry.line;
	const leadingTimestamp = line.match(LEADING_TIMESTAMP_RE);

	if (leadingTimestamp) {
		return {
			index,
			message: leadingTimestamp[2],
			raw: line,
			source: entry.source,
			timestamp: leadingTimestamp[1],
		};
	}

	const embeddedTime = line.match(EMBEDDED_TIME_RE);

	return {
		index,
		message: line,
		raw: line,
		source: entry.source,
		timestamp: embeddedTime?.[1] ?? embeddedTime?.[2],
	};
}

export function orderedLogLines(
	lines: LogLineInput[],
	latestFirst: boolean,
): ParsedLogLine[] {
	const parsed = lines.map((line, index) => parseLogLine(line, index));

	return latestFirst ? parsed.reverse() : parsed;
}

export function logLineSearchText(line: ParsedLogLine): string {
	return [line.raw, line.source?.podName, line.source?.container]
		.filter(Boolean)
		.join(" ")
		.toLowerCase();
}

function validTimestampMs(value: string | undefined): number | undefined {
	if (!value) return undefined;
	const parsed = Date.parse(value);
	return Number.isNaN(parsed) ? undefined : parsed;
}

export function latestTimestampedLogLine(
	lines: ParsedLogLine[],
): ParsedLogLine | undefined {
	return [...lines]
		.filter((line) => validTimestampMs(line.timestamp) !== undefined)
		.sort((a, b) => {
			const timeDelta =
				(validTimestampMs(b.timestamp) ?? 0) -
				(validTimestampMs(a.timestamp) ?? 0);
			if (timeDelta !== 0) return timeDelta;
			return b.index - a.index;
		})[0];
}

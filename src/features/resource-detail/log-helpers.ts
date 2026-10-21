import type { LogLineSource } from "@/lib/types";
import { type LogStructure, parseLogStructure } from "./log-structure";

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
	structure: LogStructure;
}

export const MAX_RETAINED_LOG_LINES = 1_000;

const LEADING_TIMESTAMP_RE =
	/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(.*)$/;
const EMBEDDED_TIME_RE = /\btime=(?:"([^"]+)"|([^\s]+))/;

function entryFromLogLine(input: LogLineInput): LogLineEntry {
	return input instanceof Object ? input : { line: input };
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
			structure: parseLogStructure(leadingTimestamp[2]),
		};
	}

	const embeddedTime = line.match(EMBEDDED_TIME_RE);

	return {
		index,
		message: line,
		raw: line,
		source: entry.source,
		timestamp: embeddedTime?.[1] ?? embeddedTime?.[2],
		structure: parseLogStructure(line),
	};
}

export function appendParsedLogBatch(
	lines: ParsedLogLine[],
	inputs: LogLineInput[],
	startIndex: number,
): ParsedLogLine[] {
	const retained = lines.slice(
		Math.max(0, lines.length + inputs.length - MAX_RETAINED_LOG_LINES),
	);
	const first = Math.max(0, inputs.length - MAX_RETAINED_LOG_LINES);
	for (let index = first; index < inputs.length; index += 1) {
		retained.push(parseLogLine(inputs[index], startIndex + index));
	}
	return retained;
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

export function orderParsedLogLines(
	lines: ParsedLogLine[],
	latestFirst: boolean,
): ParsedLogLine[] {
	return latestFirst ? [...lines].reverse() : lines;
}

export function filterOrderedParsedLogLines(
	lines: ParsedLogLine[],
	filterTerm: string,
): ParsedLogLine[] {
	return filterTerm
		? lines.filter((line) => logLineSearchText(line).includes(filterTerm))
		: lines;
}

function validTimestampMs(value: string | undefined): number | undefined {
	if (!value) return undefined;
	const parsed = Date.parse(value);
	return Number.isNaN(parsed) ? undefined : parsed;
}

export function latestTimestampedLogLine(
	lines: ParsedLogLine[],
): ParsedLogLine | undefined {
	let latest: ParsedLogLine | undefined;
	let latestTime = -Infinity;
	for (const line of lines) {
		const time = validTimestampMs(line.timestamp);
		if (time === undefined) continue;
		if (time > latestTime || (time === latestTime && latest && line.index > latest.index)) {
			latest = line;
			latestTime = time;
		}
	}
	return latest;
}

export interface ParsedLogLine {
	index: number;
	message: string;
	raw: string;
	timestamp?: string;
}

const LEADING_TIMESTAMP_RE =
	/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)\s+(.*)$/;
const EMBEDDED_TIME_RE = /\btime=(?:"([^"]+)"|([^\s]+))/;

export function parseLogLine(line: string, index = 0): ParsedLogLine {
	const leadingTimestamp = line.match(LEADING_TIMESTAMP_RE);

	if (leadingTimestamp) {
		return {
			index,
			message: leadingTimestamp[2],
			raw: line,
			timestamp: leadingTimestamp[1],
		};
	}

	const embeddedTime = line.match(EMBEDDED_TIME_RE);

	return {
		index,
		message: line,
		raw: line,
		timestamp: embeddedTime?.[1] ?? embeddedTime?.[2],
	};
}

export function orderedLogLines(
	lines: string[],
	latestFirst: boolean,
): ParsedLogLine[] {
	const parsed = lines.map((line, index) => parseLogLine(line, index));

	return latestFirst ? parsed.reverse() : parsed;
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

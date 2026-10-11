import type { JsonObject, JsonValue } from "@/lib/types";

export type LogLevel = "error" | "warn" | "info" | "debug";

export interface LogField {
	key: string;
	value: string;
}

export interface LogStructure {
	level?: LogLevel;
	headline: string;
	error?: string;
	fields: LogField[];
}

export interface HighlightSegment {
	text: string;
	match: boolean;
}

interface SplitFields {
	headline: string;
	pairs: LogField[];
}

const LEVEL_ALIASES = new Map<string, LogLevel>([
	["err", "error"],
	["error", "error"],
	["fatal", "error"],
	["panic", "error"],
	["crit", "error"],
	["critical", "error"],
	["wrn", "warn"],
	["warn", "warn"],
	["warning", "warn"],
	["inf", "info"],
	["info", "info"],
	["notice", "info"],
	["dbg", "debug"],
	["debug", "debug"],
	["trc", "debug"],
	["trace", "debug"],
]);
const KLOG_LEVELS = new Map<string, LogLevel>([
	["I", "info"],
	["W", "warn"],
	["E", "error"],
	["F", "error"],
]);
const LEVEL_KEYS = new Set(["level", "lvl", "severity"]);
const MESSAGE_KEYS = new Set(["msg", "message"]);
const ERROR_KEYS = new Set(["error", "err"]);
const TIME_KEYS = new Set(["time", "ts", "timestamp", "@timestamp"]);

const LEADING_TIMESTAMP_RE =
	/^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?\s+/;
const KLOG_PREFIX_RE = /^([IWEF])\d{4} \d{2}:\d{2}:\d{2}\.\d+\s+\d+\s+[^\s\]]+:\d+\]\s?/;
const LEVEL_TOKEN_RE =
	/^\[?(err|error|fatal|panic|crit|critical|wrn|warn|warning|inf|info|notice|dbg|debug|trc|trace)\]?(?::|\s|$)\s*/i;
const FIRST_FIELD_RE = /(?:^|\s)[A-Za-z_][\w.-]*=/;
const FIELD_RE = /\s*([A-Za-z_][\w.-]*)=("(?:[^"\\]|\\.)*"|\S*)/y;
const QUOTED_RE = /^"(?:[^"\\]|\\.)*"$/;

function levelFrom(value: string): LogLevel | undefined {
	return LEVEL_ALIASES.get(value.toLowerCase());
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}

function isRecord(value: JsonValue): value is JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function unquote(value: string): string {
	if (!QUOTED_RE.test(value)) return value;
	try {
		// QUOTED_RE matched a JSON string literal, so a successful parse yields a string.
		const parsed: JsonValue = JSON.parse(value);
		return String(parsed);
	} catch {
		return value.slice(1, -1).replace(/\\(.)/g, "$1");
	}
}

function stringValue(value: JsonValue | undefined): string {
	return isString(value) ? value : JSON.stringify(value);
}

function finish(
	level: LogLevel | undefined,
	headline: string,
	message: string | undefined,
	error: string | undefined,
	fields: LogField[],
): LogStructure {
	let text = headline || message || "";
	let errorText = error;
	if (!text && errorText) {
		text = errorText;
		errorText = undefined;
	}
	const structure: LogStructure = { headline: text, fields };
	if (level) structure.level = level;
	if (errorText) structure.error = errorText;
	return structure;
}

function parseJson(text: string): LogStructure | undefined {
	let value: JsonValue;
	try {
		value = JSON.parse(text);
	} catch {
		return undefined;
	}
	if (!isRecord(value)) return undefined;
	let level: LogLevel | undefined;
	let message: string | undefined;
	let error: string | undefined;
	const fields: LogField[] = [];
	for (const [key, raw] of Object.entries(value)) {
		const lowerKey = key.toLowerCase();
		if (TIME_KEYS.has(lowerKey)) continue;
		const fieldValue = stringValue(raw);
		const mappedLevel = LEVEL_KEYS.has(lowerKey) ? levelFrom(fieldValue) : undefined;
		if (mappedLevel) level ??= mappedLevel;
		else if (MESSAGE_KEYS.has(lowerKey) && message === undefined) message = fieldValue;
		else if (ERROR_KEYS.has(lowerKey) && error === undefined) error = fieldValue;
		else fields.push({ key, value: fieldValue });
	}
	return finish(level, "", message, error, fields);
}

/** Splits trailing logfmt fields from the text; returns no fields when the tail is not pure logfmt. */
function splitFields(text: string): SplitFields {
	const start = text.search(FIRST_FIELD_RE);
	if (start < 0) return { headline: text, pairs: [] };
	const pairs: LogField[] = [];
	FIELD_RE.lastIndex = start;
	while (FIELD_RE.lastIndex < text.length) {
		const match = FIELD_RE.exec(text);
		if (!match) {
			if (text.slice(FIELD_RE.lastIndex).trim() === "") break;
			return { headline: text, pairs: [] };
		}
		pairs.push({ key: match[1], value: match[2] });
	}
	return { headline: text.slice(0, start).trim(), pairs };
}

export function parseLogStructure(message: string): LogStructure {
	const text = message.replace(LEADING_TIMESTAMP_RE, "");

	if (text.startsWith("{")) {
		const json = parseJson(text);
		if (json) return json;
		return { headline: text, fields: [] };
	}

	let level: LogLevel | undefined;
	let rest = text;
	const klog = rest.match(KLOG_PREFIX_RE);
	if (klog) {
		level = KLOG_LEVELS.get(klog[1]);
		rest = rest.slice(klog[0].length);
	} else {
		const token = rest.match(LEVEL_TOKEN_RE);
		if (token) {
			level = levelFrom(token[1]);
			rest = rest.slice(token[0].length);
		}
	}

	const { headline, pairs } = splitFields(rest);
	let msg: string | undefined;
	let error: string | undefined;
	const fields: LogField[] = [];
	for (const pair of pairs) {
		const lowerKey = pair.key.toLowerCase();
		if (TIME_KEYS.has(lowerKey)) continue;
		const value = unquote(pair.value);
		const mappedLevel = LEVEL_KEYS.has(lowerKey) ? levelFrom(value) : undefined;
		if (mappedLevel) level ??= mappedLevel;
		else if (MESSAGE_KEYS.has(lowerKey) && msg === undefined) msg = value;
		else if (ERROR_KEYS.has(lowerKey) && error === undefined) error = value;
		else fields.push({ key: pair.key, value });
	}
	return finish(level, unquote(headline.trim()), msg, error, fields);
}

export function highlightSegments(text: string, term: string): HighlightSegment[] {
	const needle = term.toLowerCase();
	if (!needle) return [{ text, match: false }];
	const haystack = text.toLowerCase();
	const segments: HighlightSegment[] = [];
	let cursor = 0;
	let found = haystack.indexOf(needle);
	while (found >= 0) {
		if (found > cursor) segments.push({ text: text.slice(cursor, found), match: false });
		segments.push({ text: text.slice(found, found + needle.length), match: true });
		cursor = found + needle.length;
		found = haystack.indexOf(needle, cursor);
	}
	if (cursor < text.length || segments.length === 0) {
		segments.push({ text: text.slice(cursor), match: false });
	}
	return segments;
}

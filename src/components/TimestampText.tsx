import type { ReactNode } from "react";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import {
	type TimestampTimezone,
	useSettingsState,
} from "@/lib/settings";

type TimestampPrecision = "minute" | "second" | "millisecond";

function pad(value: number): string {
	return String(value).padStart(2, "0");
}

function milliseconds(value: number): string {
	return String(value).padStart(3, "0");
}

function timeParts(date: Date, timezone: TimestampTimezone) {
	if (timezone === "utc") {
		return {
			year: date.getUTCFullYear(),
			month: date.getUTCMonth() + 1,
			day: date.getUTCDate(),
			hour: date.getUTCHours(),
			minute: date.getUTCMinutes(),
			second: date.getUTCSeconds(),
			millisecond: date.getUTCMilliseconds(),
			suffix: " UTC",
		};
	}

	return {
		year: date.getFullYear(),
		month: date.getMonth() + 1,
		day: date.getDate(),
		hour: date.getHours(),
		minute: date.getMinutes(),
		second: date.getSeconds(),
		millisecond: date.getMilliseconds(),
		suffix: "",
	};
}

export function formatExactTimestamp(
	value: string | null | undefined,
	timezone: TimestampTimezone,
	precision: TimestampPrecision = "minute",
): string | null {
	if (!value) return null;
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return null;

	const parts = timeParts(date, timezone);
	let time = `${pad(parts.hour)}:${pad(parts.minute)}`;

	if (precision === "second" || precision === "millisecond") {
		time = `${time}:${pad(parts.second)}`;
	}
	if (precision === "millisecond") {
		time = `${time}.${milliseconds(parts.millisecond)}`;
	}

	return `${parts.year}-${pad(parts.month)}-${pad(parts.day)} ${time}${parts.suffix}`;
}

export function formatRelativeTimestamp(
	relative: string,
	exact: string | null | undefined,
	showExact: boolean,
	timezone: TimestampTimezone,
	precision: TimestampPrecision = "minute",
): string {
	const exactTimestamp = formatExactTimestamp(exact, timezone, precision);
	if (!showExact || !exactTimestamp) return relative;
	return `${relative} (${exactTimestamp})`;
}

export function TimestampText({
	relative,
	exact,
	className,
	precision,
	children,
}: {
	relative: string;
	exact?: string | null;
	className?: string;
	precision?: TimestampPrecision;
	children?: ReactNode;
}) {
	const { showExactTimestamps, timestampTimezone } = useSettingsState();
	const exactTimestamp = formatExactTimestamp(exact, timestampTimezone, precision);
	const visible = formatRelativeTimestamp(
		relative,
		exact,
		showExactTimestamps,
		timestampTimezone,
		precision,
	);
	const content = children ?? visible;

	if (!exactTimestamp) {
		return <span className={className}>{content}</span>;
	}

	return (
		<Tooltip>
			<TooltipTrigger asChild>
				<span
					className={className}
					tabIndex={0}
				>
					{content}
				</span>
			</TooltipTrigger>
			<TooltipContent side="top" align="start" sideOffset={6}>
				{exactTimestamp}
			</TooltipContent>
		</Tooltip>
	);
}

export function ExactTimestampText({
	value,
	className,
	precision,
}: {
	value: string;
	className?: string;
	precision?: TimestampPrecision;
}) {
	const { timestampTimezone } = useSettingsState();
	const formatted = formatExactTimestamp(value, timestampTimezone, precision) ?? value;

	return (
		<TimestampText
			relative={formatted}
			exact={value}
			className={className}
			precision={precision}
		>
			{formatted}
		</TimestampText>
	);
}

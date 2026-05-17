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

function pad(value: number): string {
	return String(value).padStart(2, "0");
}

export function formatExactTimestamp(
	value: string | null | undefined,
	timezone: TimestampTimezone,
): string | null {
	if (!value) return null;
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return null;

	if (timezone === "utc") {
		return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(
			date.getUTCDate(),
		)} ${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())} UTC`;
	}

	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
		date.getDate(),
	)} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function formatRelativeTimestamp(
	relative: string,
	exact: string | null | undefined,
	showExact: boolean,
	timezone: TimestampTimezone,
): string {
	const exactTimestamp = formatExactTimestamp(exact, timezone);
	if (!showExact || !exactTimestamp) return relative;
	return `${relative} (${exactTimestamp})`;
}

export function TimestampText({
	relative,
	exact,
	className,
	children,
}: {
	relative: string;
	exact?: string | null;
	className?: string;
	children?: ReactNode;
}) {
	const { showExactTimestamps, timestampTimezone } = useSettingsState();
	const exactTimestamp = formatExactTimestamp(exact, timestampTimezone);
	const visible = formatRelativeTimestamp(
		relative,
		exact,
		showExactTimestamps,
		timestampTimezone,
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
}: {
	value: string;
	className?: string;
}) {
	const { timestampTimezone } = useSettingsState();
	const formatted = formatExactTimestamp(value, timestampTimezone) ?? value;

	return (
		<TimestampText relative={formatted} exact={value} className={className}>
			{formatted}
		</TimestampText>
	);
}

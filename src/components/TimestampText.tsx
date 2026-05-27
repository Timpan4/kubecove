import type { ReactNode } from "react";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import {
	useSettingsState,
} from "@/lib/settings";
import {
	formatExactTimestamp,
	formatRelativeTimestamp,
	type TimestampPrecision,
} from "./timestamp-format";

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

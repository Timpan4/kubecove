import { Badge } from "@/components/ui/badge";
import { cnfast } from "@/lib/utils";
import {
	CHIP_BADGE_STYLES,
	DETAIL_KEY_CLASS,
	DETAIL_ROW_CLASS,
	DETAIL_VALUE_CLASS,
	type ChipVariant,
} from "./constants";

const SUCCESS_STATUS_VALUES = new Set([
	"Running",
	"Succeeded",
	"Complete",
	"Completed",
	"Ready",
]);

export function StatusChip({
	value,
	label,
	tone,
}: {
	value: string | undefined;
	label: string;
	tone?: ChipVariant;
}) {
	if (!value) return null;
	const variant: ChipVariant =
		tone ??
		(SUCCESS_STATUS_VALUES.has(value)
			? "success"
			: value === "Pending" || value === "Terminating"
				? "warning"
				: value === "Failed" || value === "Error" || value === "Not ready"
					? "error"
					: "neutral");
	const badgeStyle = CHIP_BADGE_STYLES[variant];
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>
				<Badge
					variant={badgeStyle.variant}
					className={cnfast(
						"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
						badgeStyle.className,
					)}
				>
					{value}
				</Badge>
			</span>
		</div>
	);
}

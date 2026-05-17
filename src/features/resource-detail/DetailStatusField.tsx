import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import {
	CHIP_BADGE_STYLES,
	DETAIL_KEY_CLASS,
	DETAIL_ROW_CLASS,
	DETAIL_VALUE_CLASS,
	type ChipVariant,
} from "./constants";

export function StatusChip({
	value,
	label,
}: {
	value: string | undefined;
	label: string;
}) {
	if (!value) return null;
	const variant: ChipVariant =
		value === "Running" || value === "Succeeded" || value === "Ready"
			? "success"
			: value === "Pending" || value === "Terminating"
				? "warning"
				: value === "Failed" || value === "Error"
					? "error"
					: "neutral";
	const badgeStyle = CHIP_BADGE_STYLES[variant];
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>
				<Badge
					variant={badgeStyle.variant}
					className={cn(
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

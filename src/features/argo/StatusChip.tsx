import { StatusBadge } from "@/components/StatusBadge";
import type { ChipVariant } from "./status";

export function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string | null | undefined;
	variant?: ChipVariant;
}) {
	if (!value) return null;
	return <StatusBadge tone={variant}>{value}</StatusBadge>;
}

import { StatusBadge, type StatusTone } from "@/components/StatusBadge";

export type ChipVariant = StatusTone;

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

export function syncStatusVariant(status: string | null): ChipVariant {
	if (status === "Synced") return "success";
	if (status === "OutOfSync") return "warning";
	if (status === "Unknown") return "neutral";
	return "neutral";
}

export function healthStatusVariant(status: string | null): ChipVariant {
	if (status === "Healthy") return "success";
	if (status === "Degraded" || status === "Missing") return "error";
	if (status === "Progressing" || status === "Unknown") return "warning";
	return "neutral";
}

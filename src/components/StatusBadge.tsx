import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { STATUS_BADGE_STYLES } from "./status-badge-styles";

export type StatusTone = "neutral" | "success" | "warning" | "error" | "info";

export function StatusBadge({
	children,
	tone = "neutral",
	className,
}: {
	children: ReactNode;
	tone?: StatusTone;
	className?: string;
}) {
	const badgeStyle = STATUS_BADGE_STYLES[tone];
	return (
		<Badge
			variant={badgeStyle.variant}
			className={cn(
				"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
				badgeStyle.className,
				className,
			)}
		>
			{children}
		</Badge>
	);
}

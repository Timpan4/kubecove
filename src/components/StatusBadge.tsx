import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type StatusTone = "neutral" | "success" | "warning" | "error" | "info";

export const STATUS_BADGE_STYLES: Record<
	StatusTone,
	{
		variant: "secondary" | "destructive" | "outline";
		className: string;
	}
> = {
	neutral: {
		variant: "secondary",
		className: "",
	},
	success: {
		variant: "outline",
		className:
			"border-emerald-500/30 bg-emerald-500/10 text-emerald-300 dark:bg-emerald-500/15",
	},
	warning: {
		variant: "outline",
		className:
			"border-amber-500/30 bg-amber-500/10 text-amber-300 dark:bg-amber-500/15",
	},
	error: {
		variant: "destructive",
		className: "",
	},
	info: {
		variant: "outline",
		className:
			"border-sky-500/30 bg-sky-500/10 text-sky-300 dark:bg-sky-500/15",
	},
};

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

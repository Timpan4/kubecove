import type { StatusTone } from "./StatusBadge";

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

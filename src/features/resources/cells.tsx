import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TimestampText } from "@/components/TimestampText";
import { getResourceKindVisual } from "@/lib/resource-visuals";
import type { ResourceSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { tableTooltipText } from "./helpers";

type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";
const CHIP_BADGE_STYLES: Record<
	ChipVariant,
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

export function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string;
	variant?: ChipVariant;
}) {
	const badgeStyle = CHIP_BADGE_STYLES[variant];
	return (
		<TableTooltip content={value}>
			<Badge
				variant={badgeStyle.variant}
				className={cn(
					"max-w-full rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
					badgeStyle.className,
				)}
			>
				{value}
			</Badge>
		</TableTooltip>
	);
}

function TableTooltip({
	children,
	content,
}: {
	children: ReactNode;
	content: string;
}) {
	return (
		<Tooltip>
			<TooltipTrigger asChild>
				<span className="block min-w-0">{children}</span>
			</TooltipTrigger>
			<TooltipContent side="top" align="start" sideOffset={6}>
				{content}
			</TooltipContent>
		</Tooltip>
	);
}

export function TruncatedCell({
	value,
}: {
	value: string | number | null | undefined;
}) {
	const text = tableTooltipText(value);
	return (
		<TableTooltip content={text}>
			<span className="block min-w-0 truncate">{text}</span>
		</TableTooltip>
	);
}

export function KindCell({ kind }: { kind: string }) {
	const visual = getResourceKindVisual(kind);
	const Icon = visual.icon;
	return (
		<TableTooltip content={kind}>
			<span className="inline-flex min-w-0 max-w-full items-center gap-1.5 truncate">
				<Icon className={cn("size-3.5 shrink-0", visual.className)} />
				<span className="min-w-0 truncate">{kind}</span>
			</span>
		</TableTooltip>
	);
}

export function AgeCell({ row }: { row: ResourceSummary }) {
	return (
		<TimestampText
			relative={row.age}
			exact={row.createdAt}
			className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
		/>
	);
}

export function ArgoHelmBadges({ row }: { row: ResourceSummary }) {
	const badges: Array<{ label: string; className: string }> = [];

	if (row.argoApp) {
		badges.push({
			label: `Argo: ${row.argoApp}`,
			className:
				"border-primary/30 bg-primary/10 text-primary dark:bg-primary/15",
		});
	}

	if (row.helmRelease) {
		badges.push({
			label: `Helm: ${row.helmRelease}`,
			className:
				"border-sky-500/30 bg-sky-500/10 text-sky-300 dark:bg-sky-500/15",
		});
	}

	if (badges.length === 0) return null;

	return (
		<div className="flex flex-wrap gap-1">
			{badges.map((badge) => (
				<TableTooltip key={badge.label} content={badge.label}>
					<Badge
						variant="outline"
						className={cn(
							"max-w-full truncate rounded-sm px-1.5 py-0 text-[0.625rem] shadow-none",
							badge.className,
						)}
					>
						{badge.label}
					</Badge>
				</TableTooltip>
			))}
		</div>
	);
}

export type { ChipVariant };

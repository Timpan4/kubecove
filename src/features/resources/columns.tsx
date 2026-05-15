import type { ReactNode } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import type { ResourceSummary } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TimestampText } from "@/components/TimestampText";
import { cn } from "@/lib/utils";
import { getResourceKindVisual } from "@/lib/resource-visuals";
import { tableTooltipText } from "./helpers";

const columnHelper = createColumnHelper<ResourceSummary>();

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

function StatusChip({
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
				<span
					className="block min-w-0 outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
					tabIndex={0}
				>
					{children}
				</span>
			</TooltipTrigger>
			<TooltipContent side="top" align="start" sideOffset={6}>
				{content}
			</TooltipContent>
		</Tooltip>
	);
}

function TruncatedCell({ value }: { value: string | number | null | undefined }) {
	const text = tableTooltipText(value);
	return (
		<TableTooltip content={text}>
			<span className="block min-w-0 truncate">{text}</span>
		</TableTooltip>
	);
}

function KindCell({ kind }: { kind: string }) {
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

function AgeCell({ row }: { row: ResourceSummary }) {
	return (
		<TimestampText
			relative={row.age}
			exact={row.createdAt}
			className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
		/>
	);
}

// Argo/Helm badges rendered inline in the App column
function ArgoHelmBadges({ row }: { row: ResourceSummary }) {
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
			{badges.map((badge, i) => (
				<TableTooltip key={i} content={badge.label}>
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

export const columns = [
	columnHelper.accessor("name", {
		header: "Name",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("namespace", {
		header: "Namespace",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("kind", {
		header: "Kind",
		cell: (info) => <KindCell kind={info.getValue()} />,
	}),
	columnHelper.accessor("status", {
		header: "Status",
		cell: (info) => {
			const value = info.getValue();
			if (!value) return "—";
			const variant: ChipVariant =
				value === "Running" || value === "Succeeded" || value === "Ready"
					? "success"
					: value === "Pending" || value === "Terminating"
						? "warning"
						: value === "Failed" || value === "Error"
							? "error"
							: "neutral";
			return <StatusChip value={value} variant={variant} />;
		},
	}),
	columnHelper.accessor("ready", {
		header: "Ready",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("restarts", {
		header: "Restarts",
		cell: (info) => {
			const value = info.getValue();
			if (value === undefined || value === null) return "—";
			if (value === 0) return "0";
			const variant: ChipVariant =
				value > 5 ? "error" : value > 0 ? "warning" : "neutral";
			return <StatusChip value={String(value)} variant={variant} />;
		},
	}),
	columnHelper.accessor("ownerRef", {
		header: "Owner",
		cell: (info) => <TruncatedCell value={info.getValue()} />,
	}),
	columnHelper.accessor("age", {
		header: "Age",
		cell: (info) => <AgeCell row={info.row.original} />,
	}),
	columnHelper.display({
		id: "argo-helm",
		header: "App",
		cell: ({ row }) => <ArgoHelmBadges row={row.original} />,
		enableSorting: false,
	}),
];


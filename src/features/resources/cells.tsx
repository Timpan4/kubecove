import type { ReactNode } from "react";
import { StatusBadge, type StatusTone } from "@/components/StatusBadge";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TimestampText } from "@/components/TimestampText";
import {
	formatCpuMillicores,
	formatMemoryBytes,
} from "@/lib/resource-metrics";
import type { ResourceSummary } from "@/lib/types";
import { cnfast } from "@/lib/utils";
import { gitOpsOwnerLabel, tableTooltipText } from "./helpers";

export function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string;
	variant?: StatusTone;
}) {
	return (
		<TableTooltip content={value}>
			<StatusBadge tone={variant} className="max-w-full">
				{value}
			</StatusBadge>
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

function CenteredCell({ children }: { children: ReactNode }) {
	return <div className="flex justify-center">{children}</div>;
}

export function RestartsCell({
	value,
}: {
	value: number | null | undefined;
}) {
	if (value === undefined || value === null) {
		return (
			<CenteredCell>
				<TruncatedCell value={value} />
			</CenteredCell>
		);
	}

	if (value === 0) {
		return (
			<CenteredCell>
				<TruncatedCell value={value} />
			</CenteredCell>
		);
	}

	const variant: StatusTone =
		value > 5 ? "error" : value > 0 ? "warning" : "neutral";
	return (
		<CenteredCell>
			<StatusChip value={String(value)} variant={variant} />
		</CenteredCell>
	);
}

export function CpuCell({ value }: { value: number | null | undefined }) {
	return <TruncatedCell value={formatCpuMillicores(value)} />;
}

export function MemoryCell({ value }: { value: number | null | undefined }) {
	return <TruncatedCell value={formatMemoryBytes(value)} />;
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
	const gitOpsLabel = gitOpsOwnerLabel(row);

	if (gitOpsLabel) {
		badges.push({
			label: gitOpsLabel,
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
						className={cnfast(
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

export type { StatusTone as ChipVariant };

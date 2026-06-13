import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import type { ResourceSummary } from "../../lib/types";
import { CHIP_BADGE_STYLES, type ChipVariant } from "./constants";
import {
	resourceReadyLabel,
	resourceReadyTone,
	resourceStatusTone,
	type IncidentSignal,
} from "./helpers";
import { IncidentSignalValue } from "./IncidentSignalValue";

function toneRowClassName(tone: ChipVariant): string {
	switch (tone) {
		case "error":
			return "border-l-red-500 bg-red-500/5";
		case "warning":
			return "border-l-amber-500 bg-amber-500/5";
		case "info":
			return "border-l-sky-500 bg-sky-500/5";
		case "success":
			return "border-l-emerald-500 bg-emerald-500/5";
		case "neutral":
			return "border-l-muted bg-card";
	}
}

function SummaryBadge({
	tone,
	children,
}: {
	tone: ChipVariant;
	children: ReactNode;
}) {
	const badgeStyle = CHIP_BADGE_STYLES[tone];
	return (
		<Badge
			variant={badgeStyle.variant}
			className={cn(
				"rounded-sm px-1.5 py-0 text-[0.6875rem] shadow-none",
				badgeStyle.className,
			)}
		>
			{children}
		</Badge>
	);
}

export function IncidentSummary({
	resource,
	signals,
	eventsLoading,
	eventsError,
}: {
	resource: ResourceSummary;
	signals: IncidentSignal[];
	eventsLoading: boolean;
	eventsError: boolean;
}) {
	const topSignals = signals.slice(0, 3);
	const readyLabel = resourceReadyLabel(resource);
	const hasOwnership =
		Boolean(resource.ownerRef) ||
		Boolean(resource.argoApp) ||
		Boolean(resource.helmRelease);

	return (
		<Card size="sm" className="mb-4 border-primary/30 bg-primary/5">
			<CardContent className="flex flex-col gap-3">
				<div className="flex flex-wrap items-start justify-between gap-2">
					<div className="min-w-0">
						<div className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
							Incident summary
						</div>
						<div className="mt-1 text-sm font-semibold text-foreground [overflow-wrap:anywhere]">
							{resource.kind}/{resource.name}
						</div>
					</div>
					<div className="flex flex-wrap justify-end gap-1.5">
						{resource.status && (
							<SummaryBadge tone={resourceStatusTone(resource.status)}>
								{resource.status}
							</SummaryBadge>
						)}
						{readyLabel && (
							<SummaryBadge tone={resourceReadyTone(resource)}>
								{readyLabel}
							</SummaryBadge>
						)}
						{resource.restarts !== undefined && resource.restarts > 0 && (
							<SummaryBadge tone="warning">
								{resource.restarts} restarts
							</SummaryBadge>
						)}
					</div>
				</div>
				<div className="flex flex-col gap-2">
					{topSignals.length === 0 && (
						<div className="rounded-md border bg-card px-3 py-2 text-xs text-muted-foreground">
							{eventsLoading
								? "Checking events and status signals..."
								: eventsError
									? "Status loaded; event signals are unavailable."
									: "No active incident signals for this resource."}
						</div>
					)}
					{topSignals.map((signal) => {
						const badgeStyle = CHIP_BADGE_STYLES[signal.tone];
						return (
							<div
								key={signal.id}
								className={cn(
									"flex items-start justify-between gap-2 rounded-md border border-l-4 px-2.5 py-1.5",
									toneRowClassName(signal.tone),
								)}
							>
								<div className="min-w-0">
									<div className="text-[0.78rem] font-semibold text-foreground">
										{signal.label}
									</div>
									<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
										<IncidentSignalValue signal={signal} />
									</div>
								</div>
								<Badge
									variant={badgeStyle.variant}
									className={cn(
										"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
										badgeStyle.className,
									)}
								>
									{signal.source}
								</Badge>
							</div>
						);
					})}
				</div>
				{hasOwnership && (
					<div className="flex flex-wrap gap-1.5">
						{resource.ownerRef && (
							<Badge variant="outline" className="rounded-sm">
								Owner: {resource.ownerRef}
							</Badge>
						)}
						{resource.argoApp && (
							<Badge variant="outline" className="rounded-sm">
								Argo: {resource.argoApp}
							</Badge>
						)}
						{resource.helmRelease && (
							<Badge variant="outline" className="rounded-sm">
								Helm: {resource.helmRelease}
							</Badge>
						)}
					</div>
				)}
			</CardContent>
		</Card>
	);
}

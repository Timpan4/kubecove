import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { ResourceSummary } from "../../lib/types";
import { CHIP_BADGE_STYLES } from "./constants";
import type { IncidentSignal } from "./helpers";
import { IncidentSignalValue } from "./IncidentSignalValue";

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
							<Badge variant="outline" className="rounded-sm">
								{resource.status}
							</Badge>
						)}
						{resource.ready === "True" ? (
							<Badge
								variant="outline"
								className="rounded-sm border-emerald-500/40 bg-emerald-500/10"
							>
								Ready
							</Badge>
						) : resource.ready === "False" ? (
							<Badge
								variant="outline"
								className="rounded-sm border-red-500/40 bg-red-500/10"
							>
								Not ready
							</Badge>
						) : resource.ready ? (
							<Badge variant="outline" className="rounded-sm">
								Ready {resource.ready}
							</Badge>
						) : null}
						{resource.restarts !== undefined && resource.restarts > 0 && (
							<Badge
								variant="outline"
								className="rounded-sm border-sky-500/40 bg-sky-500/10"
							>
								{resource.restarts} restarts
							</Badge>
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
								className="flex items-start justify-between gap-2 rounded-md border bg-card px-3 py-2"
							>
								<div className="min-w-0">
									<div className="text-xs font-semibold text-foreground">
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

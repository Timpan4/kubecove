import { Badge } from "@/components/ui/badge";
import type { HealthSummary, ScopePill } from "./helpers";

function HealthMetric({
	label,
	value,
	valueClassName,
}: {
	label: string;
	value: number;
	valueClassName?: string;
}) {
	return (
		<div className="flex min-h-14 flex-col justify-center gap-1 rounded-md border bg-card p-3">
			<span className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
				{label}
			</span>
			<strong className={valueClassName}>{value}</strong>
		</div>
	);
}

export function ResourceScopePills({ pills }: { pills: ScopePill[] }) {
	return (
		<div
			className="flex min-h-8 flex-wrap items-center gap-2"
			aria-label="Current resource scope"
		>
			{pills.map((pill) => (
				<Badge
					key={pill.label}
					variant="outline"
					className="h-8 max-w-full gap-1.5 rounded-sm border-slate-700/80 bg-slate-950/45 px-2.5 text-xs shadow-none"
				>
					<span className="text-muted-foreground">{pill.label}</span>
					<strong className="min-w-0 truncate font-semibold text-foreground">
						{pill.value}
					</strong>
				</Badge>
			))}
		</div>
	);
}

export function ResourceHealthStrip({ summary }: { summary: HealthSummary }) {
	return (
		<div
			className="grid grid-cols-1 gap-2 md:grid-cols-5"
			aria-label="Resource health summary"
		>
			<HealthMetric label="Total" value={summary.total} />
			<HealthMetric
				label="Healthy"
				value={summary.healthy}
				valueClassName="text-emerald-300"
			/>
			<HealthMetric
				label="Needs attention"
				value={summary.attention}
				valueClassName="text-amber-300"
			/>
			<HealthMetric
				label="Degraded"
				value={summary.degraded}
				valueClassName="text-red-300"
			/>
			<HealthMetric
				label="Restarted"
				value={summary.restarted}
				valueClassName="text-sky-300"
			/>
		</div>
	);
}

import { Button } from "@/components/ui/button";
import { cnfast } from "@/lib/utils";
import type { HealthFilter, HealthSummary } from "./helpers";

function HealthMetric({
	label,
	value,
	hint,
	valueClassName,
	active,
	onClick,
}: {
	label: string;
	value: number;
	hint?: string;
	valueClassName?: string;
	active?: boolean;
	onClick?: () => void;
}) {
	const className = cnfast(
		"flex min-h-14 flex-col justify-center gap-1 rounded-md border bg-card p-3 text-left transition-colors",
		onClick && "cursor-pointer hover:bg-muted/50 focus-visible:ring-2 focus-visible:ring-ring/50",
		active && "border-primary/60 bg-primary/10",
	);
	// Zero counts stay muted so the eye lands on the cards that matter.
	const valueClass = value === 0 ? "text-muted-foreground/70" : valueClassName;
	const body = (
		<>
			<span className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
				{label}
			</span>
			<span className="flex items-baseline gap-1.5">
				<strong className={valueClass}>{value}</strong>
				{hint && (
					<span className="text-[0.68rem] text-muted-foreground/80">{hint}</span>
				)}
			</span>
		</>
	);
	if (onClick) {
		return (
			<button
				type="button"
				className={className}
				onClick={onClick}
				aria-pressed={active}
			>
				{body}
			</button>
		);
	}

	return <div className={className}>{body}</div>;
}

function healthFilterLabel(filter: HealthFilter): string {
	switch (filter) {
		case "healthy":
			return "Healthy";
		case "unhealthy":
			return "Unhealthy";
		case "attention":
			return "Needs attention";
		case "degraded":
			return "Degraded";
		case "restarted":
			return "Restarted";
		case "all":
			return "All";
	}
}

export function ResourceHealthStrip({
	summary,
	activeFilter,
	onFilterChange,
}: {
	summary: HealthSummary;
	activeFilter: HealthFilter;
	onFilterChange: (filter: HealthFilter) => void;
}) {
	return (
		<div
			className="grid grid-cols-1 gap-2 md:grid-cols-3 xl:grid-cols-6"
			aria-label="Resource health summary"
		>
			<HealthMetric
				label="Total"
				value={summary.total}
				hint={
					summary.untracked > 0
						? `${summary.untracked} without health checks`
						: undefined
				}
				active={activeFilter === "all"}
				onClick={() => onFilterChange("all")}
			/>
			<HealthMetric
				label="Healthy"
				value={summary.healthy}
				valueClassName="text-emerald-300"
				active={activeFilter === "healthy"}
				onClick={() => onFilterChange("healthy")}
			/>
			<HealthMetric
				label="Unhealthy"
				value={summary.attention + summary.degraded}
				valueClassName="text-amber-300"
				active={activeFilter === "unhealthy"}
				onClick={() => onFilterChange("unhealthy")}
			/>
			<HealthMetric
				label="Needs attention"
				value={summary.attention}
				valueClassName="text-amber-300"
				active={activeFilter === "attention"}
				onClick={() => onFilterChange("attention")}
			/>
			<HealthMetric
				label="Degraded"
				value={summary.degraded}
				valueClassName="text-red-300"
				active={activeFilter === "degraded"}
				onClick={() => onFilterChange("degraded")}
			/>
			<HealthMetric
				label="Restarted"
				value={summary.restarted}
				valueClassName="text-sky-300"
				active={activeFilter === "restarted"}
				onClick={() => onFilterChange("restarted")}
			/>
		</div>
	);
}

export function ActiveHealthFilterBanner({
	filter,
	onReset,
}: {
	filter: HealthFilter;
	onReset: () => void;
}) {
	if (filter === "all") return null;
	return (
		<div className="flex flex-wrap items-center justify-between gap-2 rounded-md border bg-card px-3 py-2 text-xs">
			<span className="text-muted-foreground">
				Showing <strong className="text-foreground">{healthFilterLabel(filter)}</strong> resources
			</span>
			<Button type="button" variant="ghost" size="sm" onClick={onReset}>
				Reset
			</Button>
		</div>
	);
}

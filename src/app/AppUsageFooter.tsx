import { Activity, CircleAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { createTauriClient, getAppUsageMetrics } from "@/lib/tauri";
import type { AppUsageMetrics, AppUsageMetricsBreakdown } from "@/lib/types";
import {
	formatCpuPercent,
	formatMemoryBytes,
	formatProcessCount,
	formatUsageMetrics,
} from "@/lib/usage-metrics";

const USAGE_POLL_INTERVAL_MS = 2_000;

function UsageBreakdownRow({
	item,
	child = false,
}: {
	item: AppUsageMetricsBreakdown;
	child?: boolean;
}) {
	return (
		<tr className={child ? "text-neutral-500" : "text-neutral-300"}>
			<td className="min-w-0 py-1 pr-3 align-top">
				<div className={child ? "min-w-0 pl-3" : "min-w-0"}>
					<div
						className={
							child
								? "truncate text-neutral-400"
								: "truncate font-medium text-neutral-100"
						}
					>
						{item.label}
					</div>
					{!child && (
						<div className="truncate text-[0.625rem] text-neutral-500">
							{item.description}
						</div>
					)}
				</div>
			</td>
			<td className="whitespace-nowrap px-1 py-1 text-right align-top tabular-nums">
				CPU {formatCpuPercent(item.cpuPercent)}
			</td>
			<td className="whitespace-nowrap px-1 py-1 text-right align-top tabular-nums">
				{formatMemoryBytes(item.memoryBytes)}
			</td>
			<td className="whitespace-nowrap py-1 pl-1 text-right align-top tabular-nums">
				{formatProcessCount(item.processCount)}
			</td>
		</tr>
	);
}

function UsageBreakdownRows({
	item,
	depth = 0,
}: {
	item: AppUsageMetricsBreakdown;
	depth?: number;
}) {
	return (
		<>
			<UsageBreakdownRow item={item} child={depth > 0} />
			{item.children.map((child, index) => (
				<UsageBreakdownRows
					key={`${item.label}:${index}:${child.label}`}
					item={child}
					depth={depth + 1}
				/>
			))}
		</>
	);
}

export function AppUsageFooter({ visible }: { visible: boolean }) {
	const client = useMemo(() => createTauriClient(), []);
	const [metrics, setMetrics] = useState<AppUsageMetrics | null>(null);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (!visible) return;

		let cancelled = false;
		let inFlight = false;
		const loadMetrics = async () => {
			if (inFlight) return;
			inFlight = true;
			try {
				const nextMetrics = await getAppUsageMetrics(client);
				if (cancelled) return;
				setMetrics(nextMetrics);
				setError(null);
			} catch (err) {
				if (cancelled) return;
				setError(err instanceof Error ? err.message : String(err));
			} finally {
				inFlight = false;
			}
		};

		void loadMetrics();
		const interval = window.setInterval(loadMetrics, USAGE_POLL_INTERVAL_MS);

		return () => {
			cancelled = true;
			window.clearInterval(interval);
		};
	}, [client, visible]);

	if (!visible) return null;

	const content = error ? (
		<span className="inline-flex min-w-0 items-center gap-1.5">
			<CircleAlert className="size-3.5 shrink-0 text-muted-foreground" />
			<span className="truncate">Usage metrics unavailable</span>
		</span>
	) : (
		<span className="inline-flex min-w-0 items-center gap-1.5">
			<Activity className="size-3.5 shrink-0 text-muted-foreground" />
			<span className="truncate">
				{metrics ? formatUsageMetrics(metrics) : "Loading usage metrics..."}
			</span>
		</span>
	);

	return (
		<footer className="flex h-7 shrink-0 items-center justify-end gap-2 border-t bg-sidebar px-4 text-[0.6875rem] text-muted-foreground">
			{metrics && !error ? (
				<Tooltip>
					<TooltipTrigger asChild>
						<span className="min-w-0 cursor-default outline-none focus-visible:ring-2 focus-visible:ring-ring/40">
							{content}
						</span>
					</TooltipTrigger>
					<TooltipContent
						side="top"
						align="end"
						sideOffset={6}
						className="w-[27rem] max-w-[calc(100vw-2rem)] items-stretch gap-0 rounded-md border border-white/10 bg-neutral-950 p-0 text-neutral-50 shadow-xl"
					>
						<div className="w-full p-2 text-[0.6875rem]">
							<div className="px-1 pb-1 text-[0.625rem] font-semibold uppercase text-neutral-400">
								App process tree
							</div>
							<table className="w-full border-separate border-spacing-x-0 border-spacing-y-0.5">
								<colgroup>
									<col className="w-[48%]" />
									<col className="w-[15%]" />
									<col className="w-[16%]" />
									<col className="w-[21%]" />
								</colgroup>
								<thead>
									<tr className="text-[0.625rem] font-semibold uppercase text-neutral-500">
										<th className="px-1 pb-1 text-left">Process</th>
										<th className="px-1 pb-1 text-right">CPU</th>
										<th className="px-1 pb-1 text-right">Memory</th>
										<th className="px-1 pb-1 text-right">Count</th>
									</tr>
								</thead>
								<tbody>
									{metrics.breakdown.map((item) => (
										<UsageBreakdownRows key={item.label} item={item} />
									))}
								</tbody>
							</table>
						</div>
					</TooltipContent>
				</Tooltip>
			) : (
				content
			)}
		</footer>
	);
}

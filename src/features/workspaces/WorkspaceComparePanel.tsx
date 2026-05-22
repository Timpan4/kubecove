import { GitCompareArrows } from "lucide-react";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	buildWorkspaceCompareEntries,
	buildWorkspaceCompareSummaries,
	type SavedWorkspace,
} from "@/lib/workspaces";
import type { ResourceSummary } from "@/lib/types";

interface WorkspaceComparePanelProps {
	workspace: SavedWorkspace;
	rows: ResourceSummary[];
}

function CompareSide({
	label,
	total,
	degraded,
	attention,
}: {
	label: string;
	total: number;
	degraded: number;
	attention: number;
}) {
	return (
		<div className="min-w-0 rounded-md border bg-background/40 p-3">
			<div className="truncate text-sm font-medium">{label}</div>
			<div className="mt-2 grid grid-cols-3 gap-2 text-xs">
				<span className="text-muted-foreground">Total</span>
				<span className="text-muted-foreground">Warn</span>
				<span className="text-muted-foreground">Bad</span>
				<strong>{total}</strong>
				<strong className="text-amber-300">{attention}</strong>
				<strong className="text-red-300">{degraded}</strong>
			</div>
		</div>
	);
}

export function WorkspaceComparePanel({
	workspace,
	rows,
}: WorkspaceComparePanelProps) {
	const entries = buildWorkspaceCompareEntries(workspace.scope).slice(0, 2);
	if (entries.length === 0) return null;
	const summaries = buildWorkspaceCompareSummaries(entries, rows);

	return (
		<Card>
			<CardHeader>
				<CardTitle className="inline-flex items-center gap-2">
					<GitCompareArrows className="size-4" />
					Compare
				</CardTitle>
				<CardDescription>Saved scope pairs</CardDescription>
			</CardHeader>
			<CardContent className="grid gap-3">
				{summaries.map(({ entry, left, right }) => (
					<div key={entry.id} className="grid gap-2">
						<div className="text-xs font-medium text-muted-foreground">
							{entry.label}
						</div>
						<div className="grid gap-2 md:grid-cols-2">
							<CompareSide
								label={entry.leftLabel}
								total={left.total}
								attention={left.attention}
								degraded={left.degraded}
							/>
							<CompareSide
								label={entry.rightLabel}
								total={right.total}
								attention={right.attention}
								degraded={right.degraded}
							/>
						</div>
					</div>
				))}
			</CardContent>
		</Card>
	);
}

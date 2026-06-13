import { ExactTimestampText } from "@/components/TimestampText";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import {
	buildIncidentTimeline,
	type IncidentTimelineItem,
	type IncidentTimelineTone,
} from "./incident-timeline";
import type { ResourceEventSummary, ResourceSummary } from "../../lib/types";
import type { ConditionRow, ContainerStatusRow } from "./helpers";
import type { ParsedLogLine } from "./log-helpers";
import { DETAIL_SECTION_CLASS, DETAIL_SECTION_TITLE_CLASS } from "./constants";

interface IncidentTimelineProps {
	resource: ResourceSummary;
	conditions: ConditionRow[];
	events: ResourceEventSummary[];
	containers?: ContainerStatusRow[];
	logLines?: ParsedLogLine[];
}

function toneClassName(tone: IncidentTimelineTone): string {
	switch (tone) {
		case "error":
			return "border-l-red-500 bg-red-500/5";
		case "warning":
			return "border-l-amber-500 bg-amber-500/5";
		case "info":
			return "border-l-sky-500 bg-sky-500/5";
		case "neutral":
			return "border-l-muted bg-card";
	}
}

function TimelineItem({ item }: { item: IncidentTimelineItem }) {
	return (
		<div
			className={cn(
				"grid gap-1.5 rounded-md border border-l-4 px-2.5 py-2",
				toneClassName(item.tone),
			)}
		>
			<div className="flex items-start justify-between gap-2">
				<div className="min-w-0">
					<div className="text-[0.78rem] font-semibold text-foreground">
						{item.title}
					</div>
					{item.detail && (
						<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
							{item.detail}
						</div>
					)}
				</div>
				<Badge variant="outline" className="rounded-sm px-1.5 py-0 text-[0.625rem]">
					{item.source}
				</Badge>
			</div>
			{item.timestamp && (
				<div className="text-[0.6875rem] text-muted-foreground">
					<ExactTimestampText value={item.timestamp} precision="millisecond" />
				</div>
			)}
		</div>
	);
}

export function IncidentTimeline({
	resource,
	conditions,
	events,
	containers,
	logLines,
}: IncidentTimelineProps) {
	const items = buildIncidentTimeline({
		resource,
		conditions,
		events,
		containers,
		logLines,
	});

	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Timeline</div>
			{items.length === 0 ? (
				<Card size="sm">
					<CardContent className="text-xs text-muted-foreground">
						No incident timeline entries for this resource.
					</CardContent>
				</Card>
			) : (
				<div className="flex flex-col gap-2">
					{items.map((item) => (
						<TimelineItem key={item.id} item={item} />
					))}
				</div>
			)}
		</div>
	);
}

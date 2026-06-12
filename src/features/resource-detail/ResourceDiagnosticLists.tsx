import { ExactTimestampText } from "@/components/TimestampText";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import type { ResourceSummary } from "../../lib/types";
import {
	CHIP_BADGE_STYLES,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	type ChipVariant,
} from "./constants";
import {
	conditionStatusTone,
	isCleanCompletedContainer,
	type ConditionRow,
	type ContainerStatusRow,
} from "./helpers";

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

function DiagnosticSection({
	title,
	children,
}: {
	title: string;
	children: ReactNode;
}) {
	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>{title}</div>
			{children}
		</div>
	);
}

export function ConditionList({
	conditions,
	resource,
}: {
	conditions: ConditionRow[];
	resource: ResourceSummary;
}) {
	if (conditions.length === 0) return null;
	return (
		<DiagnosticSection title="Conditions">
			<div className="flex flex-col gap-2">
				{conditions.map((condition) => {
					const tone = conditionStatusTone(condition, resource);
					const badgeStyle = CHIP_BADGE_STYLES[tone];
					return (
						<div
							className={cn(
								"rounded-md border border-l-4 px-3 py-2",
								toneRowClassName(tone),
							)}
							key={`${condition.type}:${condition.status}`}
						>
							<div className="flex items-center justify-between gap-2">
								<span className="text-[0.82rem] font-semibold text-foreground">
									{condition.type}
								</span>
								<Badge
									variant={badgeStyle.variant}
									className={cn(
										"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
										badgeStyle.className,
									)}
								>
									{condition.status}
								</Badge>
							</div>
							{condition.reason && (
								<div className="mt-1 text-xs text-foreground">
									{condition.reason}
								</div>
							)}
							{condition.message && (
								<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
									{condition.message}
								</div>
							)}
							{condition.lastTransitionTime && (
								<div className="mt-1.5 text-xs text-muted-foreground">
									Last transition:{" "}
									<span className="text-foreground">
										<ExactTimestampText
											value={condition.lastTransitionTime}
											precision="millisecond"
											className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</span>
								</div>
							)}
						</div>
					);
				})}
			</div>
		</DiagnosticSection>
	);
}

function containerTone(container: ContainerStatusRow): ChipVariant {
	if (isCleanCompletedContainer(container)) return "neutral";
	if (container.exitCode !== undefined && container.exitCode !== 0) return "error";
	if (
		container.state === "waiting" ||
		(container.lastExitCode !== undefined && container.lastExitCode !== 0)
	) {
		return "warning";
	}
	if (container.ready === false) return "error";
	if (container.ready === true || container.state === "running") return "success";
	return "neutral";
}

function containerReadinessLabel(container: ContainerStatusRow): string {
	if (isCleanCompletedContainer(container)) return "completed";
	if (container.ready === undefined) return "ready unknown";
	return container.ready ? "ready" : "not ready";
}

export function ContainerList({
	containers,
}: {
	containers: ContainerStatusRow[];
}) {
	if (containers.length === 0) return null;
	return (
		<DiagnosticSection title="Containers">
			<div className="flex flex-col gap-2">
				{containers.map((container) => {
					const tone = containerTone(container);
					const badgeStyle = CHIP_BADGE_STYLES[tone];
					return (
						<div
							className={cn(
								"rounded-md border border-l-4 px-3 py-2",
								toneRowClassName(tone),
							)}
							key={container.name}
						>
							<div className="flex items-start justify-between gap-2">
								<div className="min-w-0">
									<div className="text-[0.82rem] font-semibold text-foreground [overflow-wrap:anywhere]">
										{container.name}
									</div>
									<div className="mt-1 text-xs text-muted-foreground">
										{container.state ?? "state unknown"}
										{container.reason ? ` · ${container.reason}` : ""}
									</div>
								</div>
								<Badge
									variant={badgeStyle.variant}
									className={cn(
										"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
										badgeStyle.className,
									)}
								>
									{containerReadinessLabel(container)}
								</Badge>
							</div>
							<div className="mt-2 grid gap-1 text-xs text-muted-foreground">
								<div>
									Restarts:{" "}
									<span className="text-foreground">{container.restartCount}</span>
								</div>
								{container.startedAt && (
									<div>
										Started:{" "}
										<span className="text-foreground">
											<ExactTimestampText
												value={container.startedAt}
												precision="millisecond"
												className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
											/>
										</span>
									</div>
								)}
								{container.lastState && (
									<div>
										Last state:{" "}
										<span className="text-foreground">
											{container.lastState}
											{container.lastReason ? ` · ${container.lastReason}` : ""}
											{container.lastExitCode !== undefined
												? ` · exit ${container.lastExitCode}`
												: ""}
										</span>
									</div>
								)}
								{container.lastFinishedAt && (
									<div>
										Last finished:{" "}
										<span className="text-foreground">
											<ExactTimestampText
												value={container.lastFinishedAt}
												precision="millisecond"
												className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
											/>
										</span>
									</div>
								)}
							</div>
						</div>
					);
				})}
			</div>
		</DiagnosticSection>
	);
}

import { MetadataBadges } from "@/components/MetadataBadges";
import { ExactTimestampText, TimestampText } from "@/components/TimestampText";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import {
	formatCpuMillicores,
	formatMemoryBytes,
} from "@/lib/resource-metrics";
import { cn } from "@/lib/utils";
import { useMemo } from "react";
import type { ConditionRow, ContainerStatusRow, IncidentSignal } from "./helpers";
import {
	buildIncidentSignals,
	formatMetadata,
	getContainerStatusRows,
	getErrorMessage,
	incidentSignalCardClassName,
	isCleanCompletedContainer,
} from "./helpers";
import type {
	ResourceDetailsFull,
	ResourceEventSummary,
	ResourceSummary,
} from "../../lib/types";
import {
	CHIP_BADGE_STYLES,
	type ChipVariant,
	DETAIL_KEY_CLASS,
	DETAIL_ROW_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	DETAIL_VALUE_CLASS,
	JSON_BLOCK_CLASS,
	LOADING_STATE_CLASS,
} from "./constants";
import { StatusChip } from "./DetailStatusField";
import { IncidentSignalValue } from "./IncidentSignalValue";
import { IncidentSummary } from "./IncidentSummary";
import { IncidentTimeline } from "./IncidentTimeline";
import type { ParsedLogLine } from "./log-helpers";

interface DetailsTabProps {
	resource: ResourceSummary;
	details: ResourceDetailsFull | undefined;
	detailsLoading: boolean;
	detailsError: boolean;
	detailsErr: unknown;
	conditionRows: ConditionRow[];
	events: ResourceEventSummary[] | undefined;
	eventsLoading: boolean;
	eventsError: boolean;
	logLines?: ParsedLogLine[];
	onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
}

function DetailField({
	label,
	value,
}: {
	label: string;
	value: string | undefined | null;
}) {
	if (!value) return null;
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>{value}</span>
		</div>
	);
}

function ConditionList({ conditions }: { conditions: ConditionRow[] }) {
	if (conditions.length === 0) return null;
	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Conditions</div>
			<div className="flex flex-col gap-2">
				{conditions.map((condition) => (
					<Card
						size="sm"
						key={`${condition.type}:${condition.status}`}
					>
						<CardContent>
						<div className="flex items-center justify-between gap-2">
							<span className="text-[0.82rem] font-semibold text-foreground">
								{condition.type}
							</span>
							<Badge
								variant={
									CHIP_BADGE_STYLES[
										condition.status === "True"
											? "success"
											: condition.status === "False"
												? "error"
												: "warning"
									].variant
								}
								className={cn(
									"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
									CHIP_BADGE_STYLES[
										condition.status === "True"
											? "success"
											: condition.status === "False"
												? "error"
												: "warning"
									].className,
								)}
							>
								{condition.status}
							</Badge>
						</div>
						{condition.reason && (
							<div className="mt-1.5 text-xs text-foreground">
								{condition.reason}
							</div>
						)}
						{condition.message && (
							<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">
								{condition.message}
							</div>
						)}
						{condition.lastTransitionTime && (
							<div className="mt-2 text-xs text-muted-foreground">
								Last transition:{" "}
								<span className="text-foreground">
									<ExactTimestampText
										value={condition.lastTransitionTime}
										className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
									/>
								</span>
							</div>
						)}
						</CardContent>
					</Card>
				))}
			</div>
		</div>
	);
}

function containerTone(container: ContainerStatusRow): ChipVariant {
	if (isCleanCompletedContainer(container)) {
		return "neutral";
	}
	if (container.exitCode !== undefined && container.exitCode !== 0) {
		return "error";
	}
	if (
		container.state === "waiting" ||
		(container.lastExitCode !== undefined && container.lastExitCode !== 0)
	) {
		return "warning";
	}
	if (container.ready === false) {
		return "error";
	}
	if (container.ready === true || container.state === "running") {
		return "success";
	}
	return "neutral";
}

function containerReadinessLabel(container: ContainerStatusRow): string {
	if (isCleanCompletedContainer(container)) return "completed";
	if (container.ready === undefined) return "ready unknown";
	return container.ready ? "ready" : "not ready";
}

function ContainerList({ containers }: { containers: ContainerStatusRow[] }) {
	if (containers.length === 0) return null;
	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Containers</div>
			<div className="flex flex-col gap-2">
				{containers.map((container) => {
					const tone = containerTone(container);
					const badgeStyle = CHIP_BADGE_STYLES[tone];
					return (
						<Card size="sm" key={container.name}>
							<CardContent>
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
													className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
												/>
											</span>
										</div>
									)}
								</div>
							</CardContent>
						</Card>
					);
				})}
			</div>
		</div>
	);
}

function SignalList({
	signals,
	eventsLoading,
	eventsError,
}: {
	signals: IncidentSignal[];
	eventsLoading: boolean;
	eventsError: boolean;
}) {
	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Signals</div>
			{signals.length === 0 && !eventsLoading && !eventsError && (
				<Card size="sm">
					<CardContent className="text-xs text-muted-foreground">
						No active incident signals for this resource.
					</CardContent>
				</Card>
			)}
			{signals.length === 0 && eventsLoading && (
				<Card size="sm">
					<CardContent className="text-xs text-muted-foreground">
						Checking selected resource events...
					</CardContent>
				</Card>
			)}
			{eventsError && (
				<div className="mb-2 rounded-md border border-amber-500/40 bg-card p-3 text-xs text-amber-200">
					Event signals unavailable.
				</div>
			)}
			{signals.length > 0 && (
				<div className="flex flex-col gap-2">
					{signals.map((signal) => {
						const badgeStyle = CHIP_BADGE_STYLES[signal.tone];
						return (
							<div
								className={incidentSignalCardClassName(signal.tone)}
								key={signal.id}
							>
								<div className="flex items-start justify-between gap-2">
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
							</div>
						);
					})}
				</div>
			)}
		</div>
	);
}

function BadgeRow({
	argoApp,
	helmRelease,
	namespace,
	onOpenHelmRelease,
}: {
	argoApp?: string;
	helmRelease?: string;
	namespace?: string | null;
	onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
}) {
	if (!argoApp && !helmRelease) return null;
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>App</span>
			<span className={DETAIL_VALUE_CLASS}>
				<div className="flex flex-wrap gap-1.5">
					{argoApp && (
						<Badge
							variant="outline"
							className="rounded-sm border-primary/30 bg-primary/10 px-1.5 py-0 text-[0.625rem] text-primary shadow-none dark:bg-primary/15"
						>
							Argo: {argoApp}
						</Badge>
					)}
					{helmRelease &&
						(onOpenHelmRelease ? (
							<button
								type="button"
								className="rounded-sm border border-sky-500/30 bg-sky-500/10 px-1.5 py-0 text-[0.625rem] text-sky-300 shadow-none transition-colors hover:bg-sky-500/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 dark:bg-sky-500/15"
								onClick={() => onOpenHelmRelease(helmRelease, namespace)}
							>
								Helm: {helmRelease}
							</button>
						) : (
							<Badge
								variant="outline"
								className="rounded-sm border-sky-500/30 bg-sky-500/10 px-1.5 py-0 text-[0.625rem] text-sky-300 shadow-none dark:bg-sky-500/15"
							>
								Helm: {helmRelease}
							</Badge>
						))}
				</div>
			</span>
		</div>
	);
}

function MetadataValue({ name, value }: { name: string; value: unknown }) {
	if (name === "Labels" || name === "Annotations") {
		return <MetadataBadges value={value} />;
	}
	if (name === "Created" && typeof value === "string") {
		return <ExactTimestampText value={value} />;
	}
	if (typeof value === "string") return value;
	return JSON.stringify(value);
}

export function DetailsTab({
	resource,
	details,
	detailsLoading,
	detailsError,
	detailsErr,
	conditionRows,
	events,
	eventsLoading,
	eventsError,
	logLines,
	onOpenHelmRelease,
}: DetailsTabProps) {
	const currentResource = useMemo(
		() => (details?.summary ? { ...resource, ...details.summary } : resource),
		[details?.summary, resource],
	);
	const containerRows = useMemo(
		() => getContainerStatusRows(details?.status),
		[details?.status],
	);
	const podDetailsLoading =
		currentResource.kind === "Pod" && detailsLoading && !details && !detailsError;
	const signalContainers =
		currentResource.kind === "Pod" && (details || podDetailsLoading)
			? containerRows
			: undefined;
	const signals = useMemo(
		() =>
			buildIncidentSignals(
				currentResource,
				conditionRows,
				events ?? [],
				signalContainers,
			),
		[currentResource, conditionRows, events, signalContainers],
	);
	const restartSignal = signals.find((signal) => signal.id === "restarts");
	const restartTone = restartSignal?.tone ?? "neutral";

	return (
		<>
			<IncidentSummary
				resource={currentResource}
				signals={signals}
				eventsLoading={eventsLoading}
				eventsError={eventsError}
			/>

			<IncidentTimeline
				resource={currentResource}
				conditions={conditionRows}
				events={events ?? []}
				containers={signalContainers}
				logLines={logLines}
			/>

			<div className="mb-4 grid grid-cols-1 gap-2">
				<Card size="sm" className="min-w-0">
					<CardContent>
					<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
						Kind
					</span>
					<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
						{currentResource.kind}
					</strong>
					</CardContent>
				</Card>
				{currentResource.apiVersion && (
					<Card size="sm" className="min-w-0">
						<CardContent>
						<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
							API Version
						</span>
						<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
							{currentResource.apiVersion}
						</strong>
						</CardContent>
					</Card>
				)}
				<Card size="sm" className="min-w-0">
					<CardContent>
					<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
						Namespace
						</span>
						<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
							{currentResource.namespace ?? "cluster-scoped"}
						</strong>
						</CardContent>
					</Card>
				{currentResource.age && (
					<Card size="sm" className="min-w-0">
						<CardContent>
						<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
							Age
						</span>
						<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
							<TimestampText
								relative={currentResource.age}
								exact={currentResource.createdAt}
								className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
							/>
						</strong>
						</CardContent>
					</Card>
				)}
			</div>

			<SignalList
				signals={signals}
				eventsLoading={eventsLoading}
				eventsError={eventsError}
			/>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Status</div>
				<StatusChip value={currentResource.status} label="Phase" />
				<StatusChip value={currentResource.ready} label="Ready" />
				{currentResource.restarts !== undefined && currentResource.restarts > 0 && (
					<div className={DETAIL_ROW_CLASS}>
						<span className={DETAIL_KEY_CLASS}>Restarts</span>
						<span className={DETAIL_VALUE_CLASS}>
							<Badge
								variant={CHIP_BADGE_STYLES[restartTone].variant}
								className={cn(
									"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
									CHIP_BADGE_STYLES[restartTone].className,
								)}
							>
								{currentResource.restarts}
							</Badge>
						</span>
					</div>
				)}
			</div>

			{currentResource.metrics && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Metrics</div>
					<DetailField
						label="CPU"
						value={formatCpuMillicores(currentResource.metrics.cpuMillicores)}
					/>
					<DetailField
						label="Memory"
						value={formatMemoryBytes(currentResource.metrics.memoryBytes)}
					/>
					<DetailField label="Sampled" value={currentResource.metrics.sampledAt} />
				</div>
			)}

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Ownership</div>
				<DetailField label="Owner" value={currentResource.ownerRef} />
				<BadgeRow
					argoApp={currentResource.argoApp}
					helmRelease={currentResource.helmRelease}
					namespace={currentResource.namespace}
					onOpenHelmRelease={onOpenHelmRelease}
				/>
			</div>

			{detailsLoading && (
				<div className={LOADING_STATE_CLASS}>
					<Spinner className="mx-auto mb-2 size-4" />
					<span>Loading details...</span>
				</div>
			)}
			{detailsError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to load details</AlertTitle>
					<AlertDescription>{getErrorMessage(detailsErr)}</AlertDescription>
				</Alert>
			)}
			{!detailsLoading && !detailsError && details && (
				<>
					<ConditionList conditions={conditionRows} />
					<ContainerList containers={containerRows} />
					{details.status && (
						<div className={DETAIL_SECTION_CLASS}>
							<div className={DETAIL_SECTION_TITLE_CLASS}>Status Details</div>
							<pre className={JSON_BLOCK_CLASS}>
								{JSON.stringify(details.status, null, 2)}
							</pre>
						</div>
					)}

					<div className={DETAIL_SECTION_CLASS}>
						<div className={DETAIL_SECTION_TITLE_CLASS}>Metadata</div>
						{formatMetadata(
							details.metadata as Record<string, unknown>,
						).map(({ key, value }) => (
							<div key={key} className={DETAIL_ROW_CLASS}>
								<span className={DETAIL_KEY_CLASS}>{key}</span>
								<span className={DETAIL_VALUE_CLASS}>
									<MetadataValue name={key} value={value} />
								</span>
							</div>
						))}
					</div>
				</>
			)}
		</>
	);
}

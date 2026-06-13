import { MetadataBadges } from "@/components/MetadataBadges";
import { ExactTimestampText, TimestampText } from "@/components/TimestampText";
import { Badge } from "@/components/ui/badge";
import {
	formatCpuMillicores,
	formatMemoryBytes,
} from "@/lib/resource-metrics";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import type { ResourceDetailsFull, ResourceSummary } from "../../lib/types";
import {
	CHIP_BADGE_STYLES,
	DETAIL_KEY_CLASS,
	DETAIL_ROW_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	DETAIL_VALUE_CLASS,
	JSON_BLOCK_CLASS,
} from "./constants";
import { StatusChip } from "./DetailStatusField";
import {
	formatMetadata,
	resourceReadyLabel,
	resourceReadyTone,
	type ConditionRow,
	type ContainerStatusRow,
	type IncidentSignal,
} from "./helpers";
import { ConditionList, ContainerList } from "./ResourceDiagnosticLists";

interface ResourceDiagnosticsProps {
	resource: ResourceSummary;
	details: ResourceDetailsFull | undefined;
	conditionRows: ConditionRow[];
	containerRows: ContainerStatusRow[];
	signals: IncidentSignal[];
	onOpenHelmRelease?: (releaseName: string, namespace?: string | null) => void;
}

function DetailField({
	label,
	value,
}: {
	label: string;
	value: ReactNode | undefined | null;
}) {
	if (value === undefined || value === null || value === "") return null;
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>{value}</span>
		</div>
	);
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
		return <ExactTimestampText value={value} precision="millisecond" />;
	}
	if (typeof value === "string") return value;
	return JSON.stringify(value);
}

export function ResourceDiagnostics({
	resource,
	details,
	conditionRows,
	containerRows,
	signals,
	onOpenHelmRelease,
}: ResourceDiagnosticsProps) {
	const restartSignal = signals.find((signal) => signal.id === "restarts");
	const restartTone = restartSignal?.tone ?? "neutral";
	const restartBadgeStyle = CHIP_BADGE_STYLES[restartTone];
	const hasOwnership =
		Boolean(resource.ownerRef) ||
		Boolean(resource.argoApp) ||
		Boolean(resource.helmRelease);

	return (
		<div className={DETAIL_SECTION_CLASS}>
			<div className={DETAIL_SECTION_TITLE_CLASS}>Diagnostics</div>
			<div className="flex flex-col gap-4">
				<DiagnosticSection title="Resource">
					<div className="grid grid-cols-2 gap-x-4 gap-y-2.5 rounded-md border bg-card p-3">
						<div className="min-w-0">
							<span className="mb-0.5 block text-[0.68rem] font-bold uppercase text-muted-foreground">
								Kind
							</span>
							<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
								{resource.kind}
							</strong>
						</div>
						<div className="min-w-0">
							<span className="mb-0.5 block text-[0.68rem] font-bold uppercase text-muted-foreground">
								Namespace
							</span>
							<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
								{resource.namespace ?? "cluster-scoped"}
							</strong>
						</div>
						{resource.apiVersion && (
							<div className="min-w-0">
								<span className="mb-0.5 block text-[0.68rem] font-bold uppercase text-muted-foreground">
									API Version
								</span>
								<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
									{resource.apiVersion}
								</strong>
							</div>
						)}
						{resource.age && (
							<div className="min-w-0">
								<span className="mb-0.5 block text-[0.68rem] font-bold uppercase text-muted-foreground">
									Age
								</span>
								<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
									<TimestampText
										relative={resource.age}
										exact={resource.createdAt}
										precision="millisecond"
										className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
									/>
								</strong>
							</div>
						)}
					</div>
				</DiagnosticSection>

				<DiagnosticSection title="Status">
					<StatusChip value={resource.status} label="Phase" />
					<StatusChip
						value={resourceReadyLabel(resource)}
						label="Ready"
						tone={resourceReadyTone(resource)}
					/>
					{resource.restarts !== undefined && resource.restarts > 0 && (
						<div className={DETAIL_ROW_CLASS}>
							<span className={DETAIL_KEY_CLASS}>Restarts</span>
							<span className={DETAIL_VALUE_CLASS}>
								<Badge
									variant={restartBadgeStyle.variant}
									className={cn(
										"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
										restartBadgeStyle.className,
									)}
								>
									{resource.restarts}
								</Badge>
							</span>
						</div>
					)}
				</DiagnosticSection>

				{resource.metrics && (
					<DiagnosticSection title="Metrics">
						<DetailField
							label="CPU"
							value={formatCpuMillicores(resource.metrics.cpuMillicores)}
						/>
						<DetailField
							label="Memory"
							value={formatMemoryBytes(resource.metrics.memoryBytes)}
						/>
						{resource.metrics.sampledAt && (
							<DetailField
								label="Sampled"
								value={
									<ExactTimestampText
										value={resource.metrics.sampledAt}
										precision="millisecond"
									/>
								}
							/>
						)}
					</DiagnosticSection>
				)}

				{hasOwnership && (
					<DiagnosticSection title="Ownership">
						<DetailField label="Owner" value={resource.ownerRef} />
						<BadgeRow
							argoApp={resource.argoApp}
							helmRelease={resource.helmRelease}
							namespace={resource.namespace}
							onOpenHelmRelease={onOpenHelmRelease}
						/>
					</DiagnosticSection>
				)}

				{details && (
					<>
						<ConditionList conditions={conditionRows} resource={resource} />
						<ContainerList containers={containerRows} />
						{details.status && (
							<DiagnosticSection title="Status Details">
								<pre className={JSON_BLOCK_CLASS}>
									{JSON.stringify(details.status, null, 2)}
								</pre>
							</DiagnosticSection>
						)}
						<DiagnosticSection title="Metadata">
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
						</DiagnosticSection>
					</>
				)}
			</div>
		</div>
	);
}

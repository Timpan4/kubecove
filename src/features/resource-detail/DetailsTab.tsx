import { MetadataBadges } from "@/components/MetadataBadges";
import { TimestampText } from "@/components/TimestampText";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ConditionRow } from "./helpers";
import { formatMetadata, getErrorMessage } from "./helpers";
import type { ResourceDetailsFull, ResourceSummary } from "../../lib/types";
import {
	CHIP_BADGE_STYLES,
	DETAIL_KEY_CLASS,
	DETAIL_ROW_CLASS,
	DETAIL_SECTION_CLASS,
	DETAIL_SECTION_TITLE_CLASS,
	DETAIL_VALUE_CLASS,
	ERROR_STATE_CLASS,
	JSON_BLOCK_CLASS,
	LOADING_SPINNER_CLASS,
	LOADING_STATE_CLASS,
	type ChipVariant,
} from "./constants";

interface DetailsTabProps {
	resource: ResourceSummary;
	details: ResourceDetailsFull | undefined;
	detailsLoading: boolean;
	detailsError: boolean;
	detailsErr: unknown;
	conditionRows: ConditionRow[];
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

function StatusChip({
	value,
	label,
}: {
	value: string | undefined;
	label: string;
}) {
	if (!value) return null;
	const variant: ChipVariant =
		value === "Running" || value === "Succeeded" || value === "Ready"
			? "success"
			: value === "Pending" || value === "Terminating"
				? "warning"
				: value === "Failed" || value === "Error"
					? "error"
					: "neutral";
	const badgeStyle = CHIP_BADGE_STYLES[variant];
	return (
		<div className={DETAIL_ROW_CLASS}>
			<span className={DETAIL_KEY_CLASS}>{label}</span>
			<span className={DETAIL_VALUE_CLASS}>
				<Badge
					variant={badgeStyle.variant}
					className={cn(
						"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
						badgeStyle.className,
					)}
				>
					{value}
				</Badge>
			</span>
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
					<div
						className="rounded-md border bg-card p-3"
						key={`${condition.type}:${condition.status}`}
					>
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
					</div>
				))}
			</div>
		</div>
	);
}

function BadgeRow({
	argoApp,
	helmRelease,
}: {
	argoApp?: string;
	helmRelease?: string;
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
					{helmRelease && (
						<Badge
							variant="outline"
							className="rounded-sm border-sky-500/30 bg-sky-500/10 px-1.5 py-0 text-[0.625rem] text-sky-300 shadow-none dark:bg-sky-500/15"
						>
							Helm: {helmRelease}
						</Badge>
					)}
				</div>
			</span>
		</div>
	);
}

export function DetailsTab({
	resource,
	details,
	detailsLoading,
	detailsError,
	detailsErr,
	conditionRows,
}: DetailsTabProps) {
	return (
		<>
			<div className="mb-4 grid grid-cols-1 gap-2">
				<div className="min-w-0 rounded-md border bg-card p-3">
					<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
						Kind
					</span>
					<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
						{resource.kind}
					</strong>
				</div>
				{resource.apiVersion && (
					<div className="min-w-0 rounded-md border bg-card p-3">
						<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
							API Version
						</span>
						<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
							{resource.apiVersion}
						</strong>
					</div>
				)}
				<div className="min-w-0 rounded-md border bg-card p-3">
					<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
						Namespace
					</span>
					<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
						{resource.namespace ?? "cluster-scoped"}
					</strong>
				</div>
				{resource.age && (
					<div className="min-w-0 rounded-md border bg-card p-3">
						<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">
							Age
						</span>
						<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">
							<TimestampText
								relative={resource.age}
								exact={resource.createdAt}
								className="outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
							/>
						</strong>
					</div>
				)}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Status</div>
				<StatusChip value={resource.status} label="Phase" />
				<StatusChip value={resource.ready} label="Ready" />
				{resource.restarts !== undefined && resource.restarts > 0 && (
					<div className={DETAIL_ROW_CLASS}>
						<span className={DETAIL_KEY_CLASS}>Restarts</span>
						<span className={DETAIL_VALUE_CLASS}>
							<Badge
								variant={
									CHIP_BADGE_STYLES[
										resource.restarts > 5 ? "error" : "warning"
									].variant
								}
								className={cn(
									"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
									CHIP_BADGE_STYLES[
										resource.restarts > 5 ? "error" : "warning"
									].className,
								)}
							>
								{resource.restarts}
							</Badge>
						</span>
					</div>
				)}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Ownership</div>
				<DetailField label="Owner" value={resource.ownerRef} />
				<BadgeRow
					argoApp={resource.argoApp}
					helmRelease={resource.helmRelease}
				/>
			</div>

			{detailsLoading && (
				<div className={LOADING_STATE_CLASS}>
					<div className={LOADING_SPINNER_CLASS}></div>
					<span>Loading details...</span>
				</div>
			)}
			{detailsError && (
				<div className={ERROR_STATE_CLASS}>
					<p>Error loading details: {getErrorMessage(detailsErr)}</p>
				</div>
			)}
			{!detailsLoading && !detailsError && details && (
				<>
					<ConditionList conditions={conditionRows} />
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
									{key === "Labels" || key === "Annotations" ? (
										<MetadataBadges value={value} />
									) : typeof value === "string" ? (
										value
									) : (
										JSON.stringify(value)
									)}
								</span>
							</div>
						))}
					</div>
				</>
			)}
		</>
	);
}

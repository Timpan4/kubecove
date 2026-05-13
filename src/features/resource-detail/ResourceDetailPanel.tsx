import { useQuery } from "@tanstack/react-query";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import {
	getResourceDetails,
	getResourceYaml,
	createTauriClient,
} from "../../lib/tauri";
import type { ResourceSummary } from "../../lib/types";
import { diagnosticLog, diagnosticResultSummary } from "../../lib/diagnostics";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

interface ResourceDetailPanelProps {
	resource: ResourceSummary;
	onClose: () => void;
}

type Tab = "details" | "yaml";
type ChipVariant = "neutral" | "success" | "warning" | "error" | "info";
const CHIP_BADGE_STYLES: Record<
	ChipVariant,
	{
		variant: "secondary" | "destructive" | "outline";
		className: string;
	}
> = {
	neutral: { variant: "secondary", className: "" },
	success: {
		variant: "outline",
		className:
			"border-emerald-500/30 bg-emerald-500/10 text-emerald-300 dark:bg-emerald-500/15",
	},
	warning: {
		variant: "outline",
		className:
			"border-amber-500/30 bg-amber-500/10 text-amber-300 dark:bg-amber-500/15",
	},
	error: { variant: "destructive", className: "" },
	info: {
		variant: "outline",
		className:
			"border-sky-500/30 bg-sky-500/10 text-sky-300 dark:bg-sky-500/15",
	},
};
const PANEL_CLASS =
	"flex h-full min-w-0 flex-col overflow-hidden border-l bg-card";
const PANEL_HEADER_CLASS =
	"flex shrink-0 items-center justify-between border-b px-4 py-3";
const PANEL_TITLE_CLASS = "truncate whitespace-nowrap text-sm font-semibold";
const PANEL_TABS_CLASS = "flex shrink-0 border-b";
const PANEL_TAB_CLASS =
	"rounded-none border-b-2 border-transparent bg-transparent px-4 py-2 text-[13px] text-muted-foreground shadow-none transition-colors hover:text-foreground data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none";
const PANEL_BODY_CLASS = "flex-1 overflow-y-auto p-4";
const DETAIL_SECTION_CLASS = "mb-4";
const DETAIL_SECTION_TITLE_CLASS =
	"mb-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground";
const DETAIL_ROW_CLASS = "flex gap-3 border-b py-1.5";
const DETAIL_KEY_CLASS = "min-w-[120px] text-xs font-medium text-muted-foreground";
const DETAIL_VALUE_CLASS = "overflow-wrap-anywhere text-xs text-foreground";
const LOADING_STATE_CLASS = "p-6 text-center text-xs text-muted-foreground";
const ERROR_STATE_CLASS = "p-6 text-center text-xs text-destructive";
const LOADING_SPINNER_CLASS =
	"mx-auto mb-2 size-4 animate-spin rounded-full border-2 border-muted border-t-primary";
const YAML_BLOCK_CLASS =
	"whitespace-pre-wrap break-normal font-mono text-xs leading-relaxed text-foreground [overflow-wrap:anywhere]";
const JSON_BLOCK_CLASS =
	"max-h-[220px] overflow-auto rounded-md border bg-background p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-foreground [overflow-wrap:anywhere]";

interface ConditionRow {
	type: string;
	status: string;
	reason?: string;
	message?: string;
}

export function shouldFetchResourceDetails(
	resource: Pick<ResourceSummary, "cluster" | "kind" | "name">,
): boolean {
	return (
		Boolean(resource.cluster) &&
		Boolean(resource.kind) &&
		Boolean(resource.name)
	);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function getConditionRows(
	status: Record<string, unknown> | undefined,
): ConditionRow[] {
	if (!status || !Array.isArray(status.conditions)) return [];
	return status.conditions.filter(isRecord).map((condition) => ({
		type: String(condition.type ?? "Condition"),
		status: String(condition.status ?? "Unknown"),
		reason:
			typeof condition.reason === "string" ? condition.reason : undefined,
		message:
			typeof condition.message === "string" ? condition.message : undefined,
	}));
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
					<div className="rounded-md border bg-card p-3" key={`${condition.type}:${condition.status}`}>
						<div className="flex items-center justify-between gap-2">
							<span className="text-[0.82rem] font-semibold text-foreground">{condition.type}</span>
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
							<div className="mt-1.5 text-xs text-foreground">{condition.reason}</div>
						)}
						{condition.message && (
							<div className="mt-1 text-xs leading-snug text-muted-foreground [overflow-wrap:anywhere]">{condition.message}</div>
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

export const ResourceDetailPanel = memo(function ResourceDetailPanel({
	resource,
	onClose,
}: ResourceDetailPanelProps) {
	const [activeTab, setActiveTab] = useState<Tab>("details");
	const client = useMemo(() => createTauriClient(), []);
	const resourceKey = `${resource.cluster}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
	const renderCountRef = useRef(0);
	renderCountRef.current += 1;

	// Reset state when viewing a different resource
	useEffect(() => {
		diagnosticLog("detail.resource.changed", {
			key: resourceKey,
			render: renderCountRef.current,
		});
		setActiveTab("details");
	}, [resourceKey]);

	useEffect(() => {
		diagnosticLog("detail.mount", { key: resourceKey });
		return () => {
			diagnosticLog("detail.unmount", { key: resourceKey });
		};
	}, [resourceKey]);

	const detailsEnabled = shouldFetchResourceDetails(resource);
	const yamlEnabled =
		activeTab === "yaml" &&
		!!resource.cluster &&
		!!resource.kind &&
		!!resource.name;

	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"resource-details",
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.details.fetch.start", { key: resourceKey });
			const result = await getResourceDetails(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
			);
			diagnosticLog("detail.details.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: detailsEnabled,
		retry: false,
	});

	const {
		data: yaml,
		isLoading: yamlLoading,
		isError: yamlError,
		error: yamlErr,
	} = useQuery({
		queryKey: [
			"resource-yaml",
			resource.cluster,
			resource.kind,
			resource.name,
			resource.namespace,
		],
		queryFn: async () => {
			const started = performance.now();
			diagnosticLog("detail.yaml.fetch.start", { key: resourceKey });
			const result = await getResourceYaml(
				client,
				resource.cluster,
				resource.kind,
				resource.name,
				resource.namespace ?? undefined,
			);
			diagnosticLog("detail.yaml.fetch.done", {
				key: resourceKey,
				ms: Math.round(performance.now() - started),
				result: diagnosticResultSummary(result),
			});
			return result;
		},
		enabled: yamlEnabled,
		retry: false,
	});

	useEffect(() => {
		diagnosticLog("detail.render", {
			key: resourceKey,
			render: renderCountRef.current,
			tab: activeTab,
			detailsEnabled,
			detailsLoading,
			yamlEnabled,
			yamlLoading,
			hasDetails: Boolean(details),
			hasYaml: Boolean(yaml),
		});
	});

	const formatMetadata = (
		metadata: Record<string, unknown>,
	): Array<{ key: string; value: unknown }> => {
		const entries: Array<{ key: string; value: unknown }> = [];
		if (metadata.name) entries.push({ key: "Name", value: metadata.name });
		if (metadata.namespace)
			entries.push({ key: "Namespace", value: metadata.namespace });
		if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
		if (metadata.resourceVersion)
			entries.push({
				key: "Resource Version",
				value: metadata.resourceVersion,
			});
		if (metadata.creationTimestamp)
			entries.push({ key: "Created", value: metadata.creationTimestamp });
		if (metadata.labels)
			entries.push({
				key: "Labels",
				value: JSON.stringify(metadata.labels, null, 2),
			});
		if (metadata.annotations)
			entries.push({
				key: "Annotations",
				value: JSON.stringify(metadata.annotations, null, 2),
			});
		return entries;
	};

	const getErrorMessage = (err: unknown): string => {
		if (err instanceof Error) return err.message;
		if (typeof err === "string") return err;
		return "Unknown error";
	};
	const conditionRows = useMemo(
		() => getConditionRows(details?.status),
		[details?.status],
	);

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{resource.name}</span>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7 text-muted-foreground"
					onClick={onClose}
					aria-label="Close panel"
				>
					<X className="size-4" />
				</Button>
			</div>
			<Tabs
				value={activeTab}
				onValueChange={(value) => {
					const tab = value as Tab;
					diagnosticLog("detail.tab.click", {
						key: resourceKey,
						tab,
					});
					setActiveTab(tab);
				}}
				className="min-h-0 flex-1 gap-0"
			>
				<div className={PANEL_TABS_CLASS}>
					<TabsList className="h-auto rounded-none bg-transparent p-0">
						<TabsTrigger className={PANEL_TAB_CLASS} value="details">
							Details
						</TabsTrigger>
						<TabsTrigger className={PANEL_TAB_CLASS} value="yaml">
							YAML
						</TabsTrigger>
					</TabsList>
				</div>
				<div className={PANEL_BODY_CLASS}>
					<TabsContent value="details" className="m-0">
					<>
						<div className="mb-4 grid grid-cols-1 gap-2">
							<div className="min-w-0 rounded-md border bg-card p-3">
								<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">Kind</span>
								<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">{resource.kind}</strong>
							</div>
							<div className="min-w-0 rounded-md border bg-card p-3">
								<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">Namespace</span>
								<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">{resource.namespace ?? "cluster-scoped"}</strong>
							</div>
							{resource.age && (
								<div className="min-w-0 rounded-md border bg-card p-3">
									<span className="mb-1 block text-[0.68rem] font-bold uppercase text-muted-foreground">Age</span>
									<strong className="block text-[0.82rem] text-foreground [overflow-wrap:anywhere]">{resource.age}</strong>
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
												{typeof value === "string"
													? value
													: JSON.stringify(value)}
											</span>
										</div>
									))}
								</div>
							</>
						)}
					</>
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
					<>
						{yamlLoading && (
							<div className={LOADING_STATE_CLASS}>
								<div className={LOADING_SPINNER_CLASS}></div>
								<span>Loading YAML...</span>
							</div>
						)}
						{yamlError && (
							<div className={ERROR_STATE_CLASS}>
								<p>Error loading YAML: {getErrorMessage(yamlErr)}</p>
							</div>
						)}
						{!yamlLoading && !yamlError && yaml && (
							<pre className={YAML_BLOCK_CLASS}>{yaml}</pre>
						)}
					</>
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
});

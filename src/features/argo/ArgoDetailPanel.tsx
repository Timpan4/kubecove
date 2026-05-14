import { useQuery } from "@tanstack/react-query";
import { useState, useMemo } from "react";
import {
	getArgoApplicationDetails,
	getArgoApplicationSetDetails,
	getArgoAppProjectDetails,
	createTauriClient,
} from "../../lib/tauri";
import type {
	ArgoApplicationSummary,
	ArgoApplicationSetSummary,
	ArgoAppProjectSummary,
} from "../../lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { MetadataBadges } from "@/components/MetadataBadges";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

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
const DETAIL_VALUE_CLASS = "min-w-0 flex-1 wrap-anywhere text-xs text-foreground";
const LOADING_STATE_CLASS = "p-6 text-center text-xs text-muted-foreground";
const ERROR_STATE_CLASS = "p-6 text-center text-xs text-destructive";
const LOADING_SPINNER_CLASS =
	"mx-auto mb-2 size-4 animate-spin rounded-full border-2 border-muted border-t-primary";
const YAML_BLOCK_CLASS =
	"whitespace-pre-wrap break-normal font-mono text-xs leading-relaxed text-foreground [overflow-wrap:anywhere]";
const JSON_BLOCK_CLASS =
	"whitespace-pre-wrap font-mono text-xs text-foreground [overflow-wrap:anywhere]";
const DETAIL_HINT_CLASS = "mb-4 text-xs text-muted-foreground";

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
	variant,
}: {
	value: string | null | undefined;
	label: string;
	variant: ChipVariant;
}) {
	if (!value) return null;
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

function formatMetadata(
	metadata: Record<string, unknown>,
): Array<{ key: string; value: unknown }> {
	const entries: Array<{ key: string; value: unknown }> = [];
	if (metadata.name) entries.push({ key: "Name", value: metadata.name });
	if (metadata.namespace)
		entries.push({ key: "Namespace", value: metadata.namespace });
	if (metadata.uid) entries.push({ key: "UID", value: metadata.uid });
	if (metadata.resourceVersion)
		entries.push({ key: "Resource Version", value: metadata.resourceVersion });
	if (metadata.creationTimestamp)
		entries.push({ key: "Created", value: metadata.creationTimestamp });
	if (metadata.labels)
		entries.push({
			key: "Labels",
			value: metadata.labels,
		});
	if (metadata.annotations)
		entries.push({
			key: "Annotations",
			value: metadata.annotations,
		});
	return entries;
}

function getErrorMessage(err: unknown): string {
	if (err instanceof Error) return err.message;
	if (typeof err === "string") return err;
	return "Unknown error";
}

// ── Application detail ───────────────────────────────────────────────────────

function ArgoApplicationDetail({ app }: { app: ArgoApplicationSummary }) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: ["argo-app-details", app.cluster, app.name, app.namespace],
		queryFn: () =>
			getArgoApplicationDetails(
				client,
				app.cluster,
				app.name,
				app.namespace ?? undefined,
			),
		enabled: !!app.cluster && !!app.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading details: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD Application in {details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Sync & Health</div>
				<StatusChip
					value={details.summary.syncStatus}
					label="Sync Status"
					variant={
						details.summary.syncStatus === "Synced"
							? "success"
							: details.summary.syncStatus === "OutOfSync"
								? "warning"
								: "neutral"
					}
				/>
				<StatusChip
					value={details.summary.healthStatus}
					label="Health Status"
					variant={
						details.summary.healthStatus === "Healthy"
							? "success"
							: details.summary.healthStatus === "Degraded" ||
									details.summary.healthStatus === "Missing"
								? "error"
								: "warning"
					}
				/>
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Destination</div>
				<DetailField
					label="Namespace"
					value={details.summary.destinationNamespace}
				/>
				<DetailField label="Server" value={details.summary.destinationServer} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Source</div>
				<DetailField label="Repository" value={details.summary.sourceRepo} />
				<DetailField label="Revision" value={details.summary.sourceRevision} />
				<DetailField label="Project" value={details.summary.project} />
			</div>

			{details.status && Object.keys(details.status).length > 0 && (
				<div className={DETAIL_SECTION_CLASS}>
					<div className={DETAIL_SECTION_TITLE_CLASS}>Status Details</div>
					<pre className={JSON_BLOCK_CLASS}>
						{JSON.stringify(details.status, null, 2)}
					</pre>
				</div>
			)}

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Metadata</div>
				{formatMetadata(details.metadata as Record<string, unknown>).map(
					({ key, value }) => (
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
					),
				)}
			</div>
		</>
	);
}

function ArgoApplicationYaml({ app }: { app: ArgoApplicationSummary }) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: ["argo-app-details", app.cluster, app.name, app.namespace],
		queryFn: () =>
			getArgoApplicationDetails(
				client,
				app.cluster,
				app.name,
				app.namespace ?? undefined,
			),
		enabled: !!app.cluster && !!app.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading YAML...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading YAML: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}

// ── ApplicationSet detail ─────────────────────────────────────────────────────

function ArgoApplicationSetDetail({
	appset,
}: {
	appset: ArgoApplicationSetSummary;
}) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"argo-appset-details",
			appset.cluster,
			appset.name,
			appset.namespace,
		],
		queryFn: () =>
			getArgoApplicationSetDetails(
				client,
				appset.cluster,
				appset.name,
				appset.namespace ?? undefined,
			),
		enabled: !!appset.cluster && !!appset.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading details: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD ApplicationSet in{" "}
				{details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Summary</div>
				<DetailField label="Project" value={details.summary.project} />
				<DetailField label="Status" value={details.summary.status} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Destination</div>
				<DetailField
					label="Namespace"
					value={details.summary.destinationNamespace}
				/>
				<DetailField label="Server" value={details.summary.destinationServer} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Source</div>
				<DetailField label="Repository" value={details.summary.sourceRepo} />
				<DetailField label="Revision" value={details.summary.sourceRevision} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Metadata</div>
				{formatMetadata(details.metadata as Record<string, unknown>).map(
					({ key, value }) => (
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
					),
				)}
			</div>
		</>
	);
}

function ArgoApplicationSetYaml({
	appset,
}: {
	appset: ArgoApplicationSetSummary;
}) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"argo-appset-details",
			appset.cluster,
			appset.name,
			appset.namespace,
		],
		queryFn: () =>
			getArgoApplicationSetDetails(
				client,
				appset.cluster,
				appset.name,
				appset.namespace ?? undefined,
			),
		enabled: !!appset.cluster && !!appset.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading YAML...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading YAML: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}

// ── AppProject detail ────────────────────────────────────────────────────────────

function ArgoAppProjectDetail({ project }: { project: ArgoAppProjectSummary }) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"argo-appproject-details",
			project.cluster,
			project.name,
			project.namespace,
		],
		queryFn: () =>
			getArgoAppProjectDetails(
				client,
				project.cluster,
				project.name,
				project.namespace ?? undefined,
			),
		enabled: !!project.cluster && !!project.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading details: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;

	return (
		<>
			<div className={DETAIL_HINT_CLASS}>
				Argo CD AppProject in {details.summary.namespace ?? "cluster-scoped"}
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Summary</div>
				<DetailField label="Description" value={details.summary.description} />
				<DetailField label="Status" value={details.summary.status} />
			</div>

			<div className={DETAIL_SECTION_CLASS}>
				<div className={DETAIL_SECTION_TITLE_CLASS}>Metadata</div>
				{formatMetadata(details.metadata as Record<string, unknown>).map(
					({ key, value }) => (
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
					),
				)}
			</div>
		</>
	);
}

function ArgoAppProjectYaml({ project }: { project: ArgoAppProjectSummary }) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: details,
		isLoading: detailsLoading,
		isError: detailsError,
		error: detailsErr,
	} = useQuery({
		queryKey: [
			"argo-appproject-details",
			project.cluster,
			project.name,
			project.namespace,
		],
		queryFn: () =>
			getArgoAppProjectDetails(
				client,
				project.cluster,
				project.name,
				project.namespace ?? undefined,
			),
		enabled: !!project.cluster && !!project.name,
	});

	if (detailsLoading) {
		return (
			<div className={LOADING_STATE_CLASS}>
				<div className={LOADING_SPINNER_CLASS}></div>
				<span>Loading YAML...</span>
			</div>
		);
	}
	if (detailsError) {
		return (
			<div className={ERROR_STATE_CLASS}>
				<p>Error loading YAML: {getErrorMessage(detailsErr)}</p>
			</div>
		);
	}
	if (!details) return null;
	return <pre className={YAML_BLOCK_CLASS}>{details.yaml}</pre>;
}

// ── Unified ArgoDetailPanel ───────────────────────────────────────────────────

export function ArgoDetailPanel({
	app,
	onClose,
}: {
	app:
		| ArgoApplicationSummary
		| ArgoApplicationSetSummary
		| ArgoAppProjectSummary;
	onClose: () => void;
}) {
	const [activeTab, setActiveTab] = useState<Tab>("details");

	// Determine item type by property shape returned from Rust summary models.
	const isAppProject = "description" in app;
	const isAppSet = !isAppProject && "status" in app;
	const isApp = !isAppProject && !isAppSet;

	const title = app.name + (app.namespace ? ` (${app.namespace})` : "");

	return (
		<div className={PANEL_CLASS}>
			<div className={PANEL_HEADER_CLASS}>
				<span className={PANEL_TITLE_CLASS}>{title}</span>
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
				onValueChange={(value) => setActiveTab(value as Tab)}
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
						{isApp && (
							<ArgoApplicationDetail app={app as ArgoApplicationSummary} />
						)}
						{isAppSet && (
							<ArgoApplicationSetDetail
								appset={app as ArgoApplicationSetSummary}
							/>
						)}
						{isAppProject && (
							<ArgoAppProjectDetail project={app as ArgoAppProjectSummary} />
						)}
					</>
					</TabsContent>
					<TabsContent value="yaml" className="m-0">
					<>
						{isApp && (
							<ArgoApplicationYaml app={app as ArgoApplicationSummary} />
						)}
						{isAppSet && (
							<ArgoApplicationSetYaml
								appset={app as ArgoApplicationSetSummary}
							/>
						)}
						{isAppProject && (
							<ArgoAppProjectYaml project={app as ArgoAppProjectSummary} />
						)}
					</>
					</TabsContent>
				</div>
			</Tabs>
		</div>
	);
}

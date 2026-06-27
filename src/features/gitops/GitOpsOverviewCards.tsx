import type { KeyboardEvent, ReactNode } from "react";
import { GitBranch, Package } from "lucide-react";
import { StatusBadge } from "@/components/StatusBadge";
import { TimestampText } from "@/components/TimestampText";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Empty,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { StatusChip } from "@/features/argo/StatusChip";
import { healthStatusVariant, syncStatusVariant } from "@/features/argo/status";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
	FluxResourceSummary,
} from "@/lib/types";
import { cnfast } from "@/lib/utils";
import type { GitOpsOverviewFilter } from "./gitops-overview-helpers";
import { fluxStatusTone } from "./flux-kinds";

type ArgoSummaryItem =
	| ArgoApplicationSummary
	| ArgoApplicationSetSummary
	| ArgoAppProjectSummary;

function resourceKey(item: { name: string; namespace: string | null }) {
	return `${item.namespace ?? "_cluster"}:${item.name}`;
}

function handleCardKey(
	event: KeyboardEvent<HTMLElement>,
	onSelect: () => void,
) {
	if (event.key !== "Enter" && event.key !== " ") return;
	event.preventDefault();
	onSelect();
}

function DetailRow({
	label,
	value,
	title,
}: {
	label: string;
	value: string | number | null | undefined;
	title?: string;
}) {
	return (
		<div className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 text-xs">
			<span className="text-muted-foreground">{label}</span>
			<span className="min-w-0 truncate text-foreground/90" title={title}>
				{value ?? "-"}
			</span>
		</div>
	);
}

export function LoadingState({ label }: { label: string }) {
	return (
		<div className="flex min-h-64 items-center justify-center text-sm text-muted-foreground">
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				{label}
			</span>
		</div>
	);
}

function SelectableCard({
	children,
	selected,
	onSelect,
}: {
	children: ReactNode;
	selected: boolean;
	onSelect: () => void;
}) {
	return (
		<Card
			tabIndex={0}
			role="button"
			aria-pressed={selected}
			onClick={onSelect}
			onKeyDown={(event) => handleCardKey(event, onSelect)}
			className={cnfast(
				"min-h-[236px] cursor-pointer border-l-4 border-l-transparent transition-colors hover:bg-accent/40 focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring/50",
				selected && "border-l-sidebar-primary bg-accent/50",
			)}
		>
			{children}
		</Card>
	);
}

function ArgoApplicationCard({
	app,
	selected,
	onSelect,
}: {
	app: ArgoApplicationSummary;
	selected: boolean;
	onSelect: () => void;
}) {
	return (
		<SelectableCard selected={selected} onSelect={onSelect}>
			<CardHeader>
				<CardTitle className="flex min-w-0 items-center gap-2">
					<GitBranch className="size-4 shrink-0 text-[var(--resource-argo)]" />
					<span className="truncate">{app.name}</span>
				</CardTitle>
				<CardDescription>{app.project ?? "No project"}</CardDescription>
			</CardHeader>
			<CardContent className="grid gap-2">
				<div className="mb-1 flex flex-wrap gap-1.5">
					<StatusChip
						value={app.healthStatus}
						variant={healthStatusVariant(app.healthStatus)}
					/>
					<StatusChip
						value={app.syncStatus}
						variant={syncStatusVariant(app.syncStatus)}
					/>
				</div>
				<DetailRow label="Repo" value={app.sourceRepo} title={app.sourceRepo ?? undefined} />
				<DetailRow label="Revision" value={app.sourceRevision} />
				<DetailRow label="Destination" value={app.destinationNamespace ?? app.destinationServer} />
				<DetailRow label="Namespace" value={app.namespace} />
				<div className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 text-xs">
					<span className="text-muted-foreground">Created</span>
					<TimestampText relative={app.age} exact={app.createdAt} />
				</div>
			</CardContent>
		</SelectableCard>
	);
}

function ArgoApplicationSetCard({
	appset,
	selected,
	onSelect,
}: {
	appset: ArgoApplicationSetSummary;
	selected: boolean;
	onSelect: () => void;
}) {
	return (
		<SelectableCard selected={selected} onSelect={onSelect}>
			<CardHeader>
				<CardTitle>{appset.name}</CardTitle>
				<CardDescription>{appset.project ?? "ApplicationSet"}</CardDescription>
			</CardHeader>
			<CardContent className="grid gap-2">
				<div className="mb-1 flex flex-wrap gap-1.5">
					<StatusChip value={appset.status} variant="neutral" />
					<StatusChip
						value={appset.healthStatus}
						variant={healthStatusVariant(appset.healthStatus)}
					/>
					<StatusChip
						value={appset.syncStatus}
						variant={syncStatusVariant(appset.syncStatus)}
					/>
				</div>
				<DetailRow label="Repo" value={appset.sourceRepo} title={appset.sourceRepo ?? undefined} />
				<DetailRow label="Revision" value={appset.sourceRevision} />
				<DetailRow label="Destination" value={appset.destinationNamespace ?? appset.destinationServer} />
				<DetailRow label="Namespace" value={appset.namespace} />
				<div className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 text-xs">
					<span className="text-muted-foreground">Created</span>
					<TimestampText relative={appset.age} exact={appset.createdAt} />
				</div>
			</CardContent>
		</SelectableCard>
	);
}

function ArgoAppProjectCard({
	project,
	selected,
	onSelect,
}: {
	project: ArgoAppProjectSummary;
	selected: boolean;
	onSelect: () => void;
}) {
	return (
		<SelectableCard selected={selected} onSelect={onSelect}>
			<CardHeader>
				<CardTitle>{project.name}</CardTitle>
				<CardDescription>{project.description ?? "AppProject"}</CardDescription>
			</CardHeader>
			<CardContent className="grid gap-2">
				<StatusChip value={project.status} variant="neutral" />
				<DetailRow label="Namespace" value={project.namespace} />
				<div className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 text-xs">
					<span className="text-muted-foreground">Created</span>
					<TimestampText relative={project.age} exact={project.createdAt} />
				</div>
			</CardContent>
		</SelectableCard>
	);
}

function FluxResourceCard({
	resource,
	selected,
	onSelect,
}: {
	resource: FluxResourceSummary;
	selected: boolean;
	onSelect: () => void;
}) {
	const source =
		resource.sourceKind && resource.sourceName
			? `${resource.sourceKind}/${resource.sourceName}`
			: null;
	return (
		<SelectableCard selected={selected} onSelect={onSelect}>
			<CardHeader>
				<CardTitle className="flex min-w-0 items-center gap-2">
					<Package className="size-4 shrink-0 text-muted-foreground" />
					<span className="truncate">{resource.name}</span>
				</CardTitle>
				<CardDescription>{resource.resourceKind.kind}</CardDescription>
			</CardHeader>
			<CardContent className="grid gap-2">
				<div className="mb-1 flex flex-wrap gap-1.5">
					{resource.readyStatus ? (
						<StatusBadge tone={fluxStatusTone(resource.readyStatus)}>
							Ready: {resource.readyStatus}
						</StatusBadge>
					) : (
						<StatusBadge tone="warning">Ready: unknown</StatusBadge>
					)}
					{resource.suspended && <StatusBadge tone="warning">Suspended</StatusBadge>}
				</div>
				<DetailRow label="Namespace" value={resource.namespace} />
				<DetailRow label="Source" value={source} />
				<DetailRow label="Revision" value={resource.lastAppliedRevision} />
				<DetailRow label="Inventory" value={resource.inventory.length} />
				<div className="grid grid-cols-[88px_minmax(0,1fr)] gap-3 text-xs">
					<span className="text-muted-foreground">Created</span>
					<TimestampText relative={resource.age} exact={resource.createdAt} />
				</div>
			</CardContent>
		</SelectableCard>
	);
}

export function GitOpsCardGrid({
	activeFilter,
	apps,
	appsets,
	projects,
	fluxRows,
	selectedGitOpsItem,
	selectedFluxResource,
	onGitOpsItemSelect,
	onOpenArgoApplicationResources,
	onFluxResourceSelect,
}: {
	activeFilter: GitOpsOverviewFilter | null;
	apps: ArgoApplicationSummary[];
	appsets: ArgoApplicationSetSummary[];
	projects: ArgoAppProjectSummary[];
	fluxRows: FluxResourceSummary[];
	selectedGitOpsItem: ArgoSummaryItem | null;
	selectedFluxResource: FluxResourceSummary | null;
	onGitOpsItemSelect: (item: ArgoSummaryItem) => void;
	onOpenArgoApplicationResources: (app: ArgoApplicationSummary) => void;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
}) {
	if (!activeFilter) return <EmptyCards title="No GitOps filter selected" />;
	if (activeFilter.key === "argo:applications") {
		return (
			<CardGrid>
				{apps.map((app) => (
					<ArgoApplicationCard
						key={resourceKey(app)}
						app={app}
						selected={
							selectedGitOpsItem?.name === app.name &&
							selectedGitOpsItem?.namespace === app.namespace
						}
						onSelect={() => onOpenArgoApplicationResources(app)}
					/>
				))}
				{apps.length === 0 && <EmptyCards title="No Argo CD Applications" />}
			</CardGrid>
		);
	}
	if (activeFilter.key === "argo:applicationSets") {
		return (
			<CardGrid>
				{appsets.map((appset) => (
					<ArgoApplicationSetCard
						key={resourceKey(appset)}
						appset={appset}
						selected={
							selectedGitOpsItem?.name === appset.name &&
							selectedGitOpsItem?.namespace === appset.namespace
						}
						onSelect={() => onGitOpsItemSelect(appset)}
					/>
				))}
				{appsets.length === 0 && <EmptyCards title="No Argo CD ApplicationSets" />}
			</CardGrid>
		);
	}
	if (activeFilter.key === "argo:appProjects") {
		return (
			<CardGrid>
				{projects.map((project) => (
					<ArgoAppProjectCard
						key={resourceKey(project)}
						project={project}
						selected={
							selectedGitOpsItem?.name === project.name &&
							selectedGitOpsItem?.namespace === project.namespace
						}
						onSelect={() => onGitOpsItemSelect(project)}
					/>
				))}
				{projects.length === 0 && <EmptyCards title="No Argo CD AppProjects" />}
			</CardGrid>
		);
	}
	const rows = fluxRows.filter(
		(row) => activeFilter.key === `flux:${row.resourceKind.kind}`,
	);
	return (
		<CardGrid>
			{rows.map((resource) => (
				<FluxResourceCard
					key={`${resource.resourceKind.kind}:${resourceKey(resource)}`}
					resource={resource}
					selected={
						selectedFluxResource?.name === resource.name &&
						selectedFluxResource?.namespace === resource.namespace &&
						selectedFluxResource?.resourceKind.kind === resource.resourceKind.kind
					}
					onSelect={() => onFluxResourceSelect(resource)}
				/>
			))}
			{rows.length === 0 && <EmptyCards title={`No ${activeFilter.label}`} />}
		</CardGrid>
	);
}

function CardGrid({ children }: { children: ReactNode }) {
	return (
		<div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
			{children}
		</div>
	);
}

function EmptyCards({ title }: { title: string }) {
	return (
		<div className="col-span-full">
			<Empty className="min-h-64">
				<EmptyHeader>
					<EmptyTitle>{title}</EmptyTitle>
				</EmptyHeader>
			</Empty>
		</div>
	);
}

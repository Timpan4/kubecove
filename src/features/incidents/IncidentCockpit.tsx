import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, ExternalLink, RotateCcw } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { StatusBadge, type StatusTone } from "@/components/StatusBadge";
import { ExactTimestampText, TimestampText } from "@/components/TimestampText";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import { createTauriClient, listIncidentCockpit } from "@/lib/tauri";
import type {
	IncidentCockpitItem,
	IncidentCockpitSummary,
	IncidentSeverity,
	ResourceSummary,
} from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspaces";
import {
	buildWorkspaceFetchKeys,
	buildWorkspaceFetchPlans,
} from "@/features/workspaces/query";
import { workspaceScopeContexts } from "@/lib/workspaces";
import {
	countIncidentItems,
	filterIncidentItems,
	groupIncidentItems,
	type IncidentFilter,
} from "./helpers";

interface IncidentCockpitProps {
	workspace: SavedWorkspace;
	onResourceSelect: (resource: ResourceSummary) => void;
	onOpenResources: () => void;
}

const STATE_CLASS =
	"flex min-h-64 items-center justify-center p-8 text-center text-sm text-muted-foreground";
const EMPTY_INCIDENT_ITEMS: IncidentCockpitItem[] = [];

const FILTERS: Array<{
	id: IncidentFilter;
	label: string;
	getCount: (summary: ReturnType<typeof countIncidentItems>) => number;
}> = [
	{ id: "all", label: "All", getCount: (summary) => summary.total },
	{
		id: "degraded",
		label: "Degraded",
		getCount: (summary) => summary.degraded,
	},
	{
		id: "attention",
		label: "Needs attention",
		getCount: (summary) => summary.attention,
	},
	{
		id: "restarted",
		label: "Restarted",
		getCount: (summary) => summary.restarted,
	},
	{ id: "warning", label: "Warnings", getCount: (summary) => summary.warning },
];

function severityTone(severity: IncidentSeverity): StatusTone {
	if (severity === "degraded") return "error";
	if (severity === "attention") return "warning";
	if (severity === "restarted") return "info";
	return "neutral";
}

function severityLabel(severity: IncidentSeverity): string {
	if (severity === "degraded") return "Degraded";
	if (severity === "attention") return "Needs attention";
	if (severity === "restarted") return "Restarted";
	return "Warning";
}

async function loadIncidentCockpit(
	workspace: SavedWorkspace,
	kubeconfigEnvVar: string,
): Promise<IncidentCockpitSummary> {
	const client = createTauriClient();
	const plans = buildWorkspaceFetchPlans(workspace.scope);
	const summaries = await Promise.all(
		plans.map((plan) =>
			listIncidentCockpit(
				client,
				plan.clusterContext,
				plan.requests,
				kubeconfigEnvVar,
			),
		),
	);
	return {
		cluster: workspace.scope.clusterContext,
		generatedAt: new Date().toISOString(),
		requestedScope: summaries.flatMap((summary) => summary.requestedScope),
		items: summaries.flatMap((summary) => summary.items),
		warnings: summaries.flatMap((summary) =>
			summary.warnings.map((warning) => `${summary.cluster}: ${warning}`),
		),
	};
}

function LoadingState() {
	return (
		<div className={STATE_CLASS}>
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				Loading incident cockpit...
			</span>
		</div>
	);
}

function ErrorState({ error }: { error: unknown }) {
	return (
		<div className="p-4">
			<Alert variant="destructive">
				<AlertTitle>Failed to load incident cockpit</AlertTitle>
				<AlertDescription>
					{error instanceof Error ? error.message : "Incident data unavailable"}
				</AlertDescription>
			</Alert>
		</div>
	);
}

function CockpitEmptyState({
	onOpenResources,
	scopeLabel,
}: {
	onOpenResources: () => void;
	scopeLabel: string;
}) {
	return (
		<Empty className="min-h-64 border-0">
			<EmptyHeader>
				<EmptyTitle>No active incident signals</EmptyTitle>
				<EmptyDescription>
					No degraded, restarted, or warning resources while {scopeLabel}.
					Argo CD application health is tracked separately in the Argo CD view.
				</EmptyDescription>
			</EmptyHeader>
			<Button type="button" variant="outline" onClick={onOpenResources}>
				<ExternalLink data-icon="inline-start" />
				Open resources
			</Button>
		</Empty>
	);
}

function FilterBar({
	filter,
	counts,
	onFilterChange,
}: {
	filter: IncidentFilter;
	counts: ReturnType<typeof countIncidentItems>;
	onFilterChange: (filter: IncidentFilter) => void;
}) {
	return (
		<div className="flex flex-wrap gap-2">
			{FILTERS.map((item) => {
				const active = filter === item.id;
				return (
					<Button
						key={item.id}
						type="button"
						variant={active ? "default" : "outline"}
						size="sm"
						onClick={() => onFilterChange(item.id)}
					>
						{item.label}
						<Badge variant="secondary" className="ml-1 rounded-sm px-1.5">
							{item.getCount(counts)}
						</Badge>
					</Button>
				);
			})}
		</div>
	);
}

function SignalList({ item }: { item: IncidentCockpitItem }) {
	const visible = item.signals.slice(0, 3);
	if (visible.length === 0) {
		return <span className="text-muted-foreground">No signal details</span>;
	}
	return (
		<div className="flex min-w-0 flex-col gap-1">
			{visible.map((signal, index) => (
				<div
					key={`${signal.kind}:${signal.label}:${index}`}
					className="min-w-0 text-xs"
				>
					<span className="font-medium">{signal.label}</span>
					{signal.message && (
						<span className="text-muted-foreground">
							{" "}
							{signal.message}
						</span>
					)}
				</div>
			))}
			{item.signals.length > visible.length && (
				<span className="text-xs text-muted-foreground">
					+{item.signals.length - visible.length} more signals
				</span>
			)}
		</div>
	);
}

function IncidentRow({
	item,
	onResourceSelect,
}: {
	item: IncidentCockpitItem;
	onResourceSelect: (resource: ResourceSummary) => void;
}) {
	const resource = item.resource;
	return (
		<TableRow className="align-top">
			<TableCell className="min-w-44">
				<div className="min-w-0">
					<button
						type="button"
						className="min-w-0 text-left font-medium text-foreground hover:underline focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
						onClick={() => onResourceSelect(resource)}
					>
						<span className="block truncate">
							{resource.kind}/{resource.name}
						</span>
					</button>
					<div className="mt-1 flex flex-wrap gap-1">
						{resource.status && (
							<Badge variant="outline" className="rounded-sm">
								{resource.status}
							</Badge>
						)}
						{resource.ready && (
							<Badge variant="outline" className="rounded-sm">
								Ready {resource.ready}
							</Badge>
						)}
						{resource.restarts !== undefined && resource.restarts > 0 && (
							<Badge variant="outline" className="rounded-sm">
								{resource.restarts} restarts
							</Badge>
						)}
					</div>
				</div>
			</TableCell>
			<TableCell>
				<StatusBadge tone={severityTone(item.severity)}>
					{severityLabel(item.severity)}
				</StatusBadge>
			</TableCell>
			<TableCell>
				<div className="min-w-0 text-sm">
					<div className="truncate">{resource.namespace ?? "cluster"}</div>
					<div className="truncate text-xs text-muted-foreground">
						{resource.cluster}
					</div>
				</div>
			</TableCell>
			<TableCell className="max-w-[28rem]">
				<SignalList item={item} />
			</TableCell>
			<TableCell>
				{item.latestWarningEvent ? (
					<div className="min-w-0 text-xs">
						<div className="truncate font-medium">
							{item.latestWarningEvent.reason}
						</div>
						<TimestampText
							relative={item.latestWarningEvent.lastSeen}
							exact={item.latestWarningEvent.lastSeenAt}
							className="text-muted-foreground"
						/>
					</div>
				) : (
					<span className="text-muted-foreground">-</span>
				)}
			</TableCell>
			<TableCell className="text-right">
				<Button
					type="button"
					variant="ghost"
					size="sm"
					onClick={() => onResourceSelect(resource)}
				>
					<ExternalLink data-icon="inline-start" />
					Details
				</Button>
			</TableCell>
		</TableRow>
	);
}

function IncidentTable({
	items,
	onResourceSelect,
}: {
	items: IncidentCockpitItem[];
	onResourceSelect: (resource: ResourceSummary) => void;
}) {
	const groups = groupIncidentItems(items);
	return (
		<Table className="w-full table-fixed">
			<TableHeader>
				<TableRow>
					<TableHead className="w-[23%]">Resource</TableHead>
					<TableHead className="w-[12%]">Severity</TableHead>
					<TableHead className="w-[16%]">Scope</TableHead>
					<TableHead>Signals</TableHead>
					<TableHead className="w-[14%]">Latest warning</TableHead>
					<TableHead className="w-[9%] text-right">Open</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{groups.map((group) => [
					<TableRow key={`group:${group.label}`} className="bg-muted/35">
						<TableCell
							colSpan={6}
							className="py-2 text-xs font-semibold uppercase text-muted-foreground"
						>
							{group.label} ({group.items.length})
						</TableCell>
					</TableRow>,
					...group.items.map((item) => (
						<IncidentRow
							key={`${item.resource.cluster}:${item.resource.kind}:${item.resource.namespace ?? ""}:${item.resource.name}`}
							item={item}
							onResourceSelect={onResourceSelect}
						/>
					)),
				])}
			</TableBody>
		</Table>
	);
}

export function IncidentCockpit({
	workspace,
	onResourceSelect,
	onOpenResources,
}: IncidentCockpitProps) {
	const [filter, setFilter] = useState<IncidentFilter>("all");
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigEnvVar);
	const fetchKeys = useMemo(
		() => buildWorkspaceFetchKeys(workspace.scope),
		[workspace.scope],
	);
	const workspaceContextKey = useMemo(
		() => workspaceScopeContexts(workspace.scope).join("|"),
		[workspace.scope],
	);
	const {
		data,
		isPending,
		isError,
		error,
		refetch,
		isFetching,
	} = useQuery({
		queryKey: queryKeys.incidentCockpit(
			workspaceContextKey,
			fetchKeys,
			kubeconfigEnvVar,
		),
		queryFn: () => loadIncidentCockpit(workspace, kubeconfigEnvVar),
		enabled: fetchKeys.length > 0,
		staleTime: 30_000,
	});
	const items = data?.items ?? EMPTY_INCIDENT_ITEMS;
	const counts = useMemo(() => countIncidentItems(items), [items]);
	const filteredItems = useMemo(
		() => filterIncidentItems(items, filter),
		[filter, items],
	);

	if (fetchKeys.length === 0) {
		return (
			<div className={STATE_CLASS}>
				Workspace scope has no resource kinds to inspect.
			</div>
		);
	}
	if (isPending) return <LoadingState />;
	if (isError) return <ErrorState error={error} />;
	if (!data) return <LoadingState />;

	return (
		<div className="mx-auto flex w-full max-w-7xl flex-col gap-4">
			<div className="flex flex-wrap items-start justify-between gap-3 border-b pb-3">
				<div className="min-w-0">
					<h1 className="truncate text-lg font-semibold">Incident Cockpit</h1>
					<div className="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
						<span>{workspace.name}</span>
						<span>{items.length} active signals</span>
						<span>
							{workspace.scope.namespaces.length === 0
								? "scanning all namespaces"
								: `scanning ${workspace.scope.namespaces.join(", ")}`}
						</span>
						<ExactTimestampText value={data.generatedAt} />
					</div>
				</div>
				<div className="flex flex-wrap gap-2">
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => refetch()}
						disabled={isFetching}
					>
						{isFetching ? (
							<Spinner data-icon="inline-start" />
						) : (
							<RotateCcw data-icon="inline-start" />
						)}
						Refresh
					</Button>
					<Button type="button" variant="outline" size="sm" onClick={onOpenResources}>
						<ExternalLink data-icon="inline-start" />
						Resources
					</Button>
				</div>
			</div>

			{data.warnings.length > 0 && (
				<Alert>
					<AlertTriangle className="size-3.5" />
					<AlertTitle>Partial incident data</AlertTitle>
					<AlertDescription>{data.warnings.join(" ")}</AlertDescription>
				</Alert>
			)}

			<Card>
				<CardHeader>
					<CardTitle>Signals</CardTitle>
				</CardHeader>
				<CardContent className="flex flex-col gap-3">
					<FilterBar
						filter={filter}
						counts={counts}
						onFilterChange={setFilter}
					/>
					{items.length === 0 ? (
						<CockpitEmptyState
							onOpenResources={onOpenResources}
							scopeLabel={
								workspace.scope.namespaces.length === 0
									? "scanning all namespaces"
									: `scanning ${workspace.scope.namespaces.join(", ")}`
							}
						/>
					) : filteredItems.length === 0 ? (
						<Empty className="min-h-52 border-0">
							<EmptyHeader>
								<EmptyTitle>No matching incident signals</EmptyTitle>
								<EmptyDescription>
									Change severity filter to see other active signals.
								</EmptyDescription>
							</EmptyHeader>
						</Empty>
					) : (
						<div className="min-w-0 overflow-x-auto">
							<IncidentTable
								items={filteredItems}
								onResourceSelect={onResourceSelect}
							/>
						</div>
					)}
				</CardContent>
			</Card>
		</div>
	);
}

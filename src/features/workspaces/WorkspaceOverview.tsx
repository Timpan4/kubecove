import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Boxes, FolderOpen, GitBranch, Layers } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { queryKeys } from "@/lib/queryKeys";
import {
	createTauriClient,
	detectArgoCD,
	listArgoApplications,
	listKubeContexts,
	listNamespaces,
	listResourceKinds,
} from "@/lib/tauri";
import {
	buildWorkspaceHealthSummary,
	computeRestoreStatus,
	resourceKindLabel,
	type SavedWorkspace,
	type WorkspaceShortcut,
} from "@/lib/workspaces";
import type { HealthFilter } from "@/features/resources/helpers";
import { buildWorkspaceFetchKeys, fetchWorkspaceResources } from "./query";

interface WorkspaceOverviewProps {
	workspace: SavedWorkspace;
	onOpenResources: (namespace?: string, healthFilter?: HealthFilter) => void;
	onOpenArgo: (argoApp?: string) => void;
	onOpenLauncher: () => void;
}

function Metric({
	label,
	value,
	tone,
}: {
	label: string;
	value: number | string;
	tone?: string;
}) {
	return (
		<Card size="sm" className="min-h-20 justify-center">
			<CardContent className="flex flex-col gap-1">
			<span className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
				{label}
			</span>
			<strong className={tone}>{value}</strong>
			</CardContent>
		</Card>
	);
}

function ShortcutButton({
	shortcut,
	onOpenResources,
	onOpenArgo,
}: {
	shortcut: WorkspaceShortcut;
	onOpenResources: (namespace?: string, healthFilter?: HealthFilter) => void;
	onOpenArgo: (argoApp?: string) => void;
}) {
	const icon =
		shortcut.kind === "argo" ? <GitBranch /> : shortcut.kind === "namespace" ? <Layers /> : <Boxes />;
	return (
		<Button
			type="button"
			variant="outline"
			size="sm"
			onClick={() =>
				shortcut.kind === "argo"
					? onOpenArgo(shortcut.argoApp)
					: onOpenResources(shortcut.namespace)
			}
		>
			{icon}
			{shortcut.label}
		</Button>
	);
}

function IncidentShortcutButton({
	label,
	count,
	filter,
	onOpenResources,
}: {
	label: string;
	count: number;
	filter: HealthFilter;
	onOpenResources: (namespace?: string, healthFilter?: HealthFilter) => void;
}) {
	if (count === 0) return null;
	return (
		<Button
			type="button"
			variant="outline"
			size="sm"
			onClick={() => onOpenResources(undefined, filter)}
		>
			<AlertTriangle data-icon="inline-start" />
			{label}
			<Badge variant="secondary" className="ml-1 rounded-sm px-1.5">
				{count}
			</Badge>
		</Button>
	);
}

export function WorkspaceOverview({
	workspace,
	onOpenResources,
	onOpenArgo,
	onOpenLauncher,
}: WorkspaceOverviewProps) {
	const client = useMemo(() => createTauriClient(), []);
	const contextsQuery = useQuery({
		queryKey: queryKeys.kubeContexts(),
		queryFn: () => listKubeContexts(client),
	});
	const contexts = contextsQuery.data ?? [];
	const clusterAvailable =
		contextsQuery.isPending ||
		contexts.some((context) => context.name === workspace.scope.clusterContext);

	const namespacesQuery = useQuery({
		queryKey: queryKeys.namespaces(workspace.scope.clusterContext),
		queryFn: () => listNamespaces(client, workspace.scope.clusterContext),
		enabled: clusterAvailable && !contextsQuery.isPending,
	});
	const namespaces = namespacesQuery.data?.map((namespace) => namespace.name) ?? [];

	const kindsQuery = useQuery({
		queryKey: queryKeys.resourceKinds(workspace.scope.clusterContext),
		queryFn: () => listResourceKinds(client, workspace.scope.clusterContext),
		enabled: clusterAvailable && !contextsQuery.isPending,
	});
	const discoveredKinds = kindsQuery.data ?? [];

	const restoreStatus = useMemo(
		() =>
			computeRestoreStatus(
				workspace,
				contexts,
				namespaces,
				discoveredKinds,
			),
		[workspace, contexts, namespaces, discoveredKinds],
	);
	const availableNamespaces = workspace.scope.namespaces.filter(
		(namespace) => !restoreStatus.missingNamespaces.includes(namespace),
	);
	const workspaceFetchKeys = useMemo(
		() => buildWorkspaceFetchKeys(workspace.scope, availableNamespaces),
		[workspace.scope, availableNamespaces],
	);

	const resourcesQuery = useQuery({
		queryKey: queryKeys.resources(
			workspace.scope.clusterContext,
			workspaceFetchKeys,
		),
		queryFn: () => fetchWorkspaceResources(workspace.scope, availableNamespaces),
		enabled:
			restoreStatus.clusterAvailable &&
			!namespacesQuery.isPending &&
			!kindsQuery.isPending,
		staleTime: 30_000,
	});
	const rows = resourcesQuery.data ?? [];
	const health = useMemo(() => buildWorkspaceHealthSummary(rows), [rows]);

	const argoDetectedQuery = useQuery({
		queryKey: queryKeys.argoDetect(workspace.scope.clusterContext),
		queryFn: () => detectArgoCD(client, workspace.scope.clusterContext),
		enabled: restoreStatus.clusterAvailable,
	});
	const argoAppsQuery = useQuery({
		queryKey: queryKeys.argoApps(workspace.scope.clusterContext),
		queryFn: () => listArgoApplications(client, workspace.scope.clusterContext),
		enabled: argoDetectedQuery.data === true,
	});
	const argoApps = argoAppsQuery.data;
	const argoDrift = (argoApps ?? []).filter(
		(app) => app.syncStatus && app.syncStatus !== "Synced",
	).length;
	const unhealthyCount = health.attention + health.degraded;
	const hasIncidentShortcuts =
		unhealthyCount > 0 || health.attention > 0 || health.restarted > 0;

	const hasRestoreWarning =
		!restoreStatus.clusterAvailable ||
		restoreStatus.missingNamespaces.length > 0 ||
		restoreStatus.missingKinds.length > 0;

	return (
		<div className="mx-auto flex w-full max-w-6xl flex-col gap-4">
			<div className="flex flex-wrap items-center justify-between gap-3 border-b pb-3">
				<div className="min-w-0">
					<h1 className="truncate text-lg font-semibold">{workspace.name}</h1>
					<div className="mt-1 flex flex-wrap gap-2">
						<Badge variant="outline" className="rounded-sm">
							{workspace.scope.clusterContext}
						</Badge>
						<Badge variant="outline" className="rounded-sm">
							{workspace.scope.namespaces.length || "All"} namespaces
						</Badge>
						<Badge variant="outline" className="rounded-sm">
							{workspace.scope.kinds.map(resourceKindLabel).join(", ")}
						</Badge>
					</div>
				</div>
				<div className="flex gap-2">
					<Button type="button" variant="outline" onClick={onOpenLauncher}>
						<FolderOpen data-icon="inline-start" />
						Workspaces
					</Button>
					<Button type="button" onClick={() => onOpenResources()}>
						<Boxes data-icon="inline-start" />
						Resources
					</Button>
				</div>
			</div>

			{hasRestoreWarning && (
				<Alert className="border-amber-500/40 text-amber-200">
					<AlertTriangle className="size-3.5" />
					<AlertTitle>Unavailable saved scope</AlertTitle>
					<AlertDescription className="text-amber-200/90">
					{!restoreStatus.clusterAvailable && (
						<div>Context missing: {workspace.scope.clusterContext}</div>
					)}
					{restoreStatus.missingNamespaces.length > 0 && (
						<div>Namespaces: {restoreStatus.missingNamespaces.join(", ")}</div>
					)}
					{restoreStatus.missingKinds.length > 0 && (
						<div>Kinds: {restoreStatus.missingKinds.join(", ")}</div>
					)}
					</AlertDescription>
				</Alert>
			)}

			<div className="grid grid-cols-1 gap-2 md:grid-cols-5">
				<Metric label="Total" value={health.total} />
				<Metric label="Healthy" value={health.healthy} tone="text-emerald-300" />
				<Metric label="Needs attention" value={health.attention} tone="text-amber-300" />
				<Metric label="Degraded" value={health.degraded} tone="text-red-300" />
				<Metric label="Restarted" value={health.restarted} tone="text-sky-300" />
			</div>

			{hasIncidentShortcuts && (
				<Card>
					<CardHeader>
						<CardTitle>Incident shortcuts</CardTitle>
						<CardDescription>
							Open the saved scope with an investigation filter applied.
						</CardDescription>
					</CardHeader>
					<CardContent className="flex flex-wrap gap-2">
						<IncidentShortcutButton
							label="Unhealthy"
							count={unhealthyCount}
							filter="unhealthy"
							onOpenResources={onOpenResources}
						/>
						<IncidentShortcutButton
							label="Warnings"
							count={health.attention}
							filter="attention"
							onOpenResources={onOpenResources}
						/>
						<IncidentShortcutButton
							label="Restarted"
							count={health.restarted}
							filter="restarted"
							onOpenResources={onOpenResources}
						/>
					</CardContent>
				</Card>
			)}

			<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
				<Card>
					<CardHeader>
						<CardTitle>Shortcuts</CardTitle>
						<span className="text-xs text-muted-foreground">
							{workspace.shortcuts.length}
						</span>
					</CardHeader>
					<CardContent className="flex flex-wrap gap-2">
						{workspace.shortcuts.length === 0 && (
							<Empty className="min-h-32 border-0">
								<EmptyHeader>
									<EmptyTitle>No shortcuts</EmptyTitle>
									<EmptyDescription>
										Workspace shortcuts appear after scope is saved.
									</EmptyDescription>
								</EmptyHeader>
							</Empty>
						)}
						{workspace.shortcuts.map((shortcut) => (
							<ShortcutButton
								key={shortcut.id}
								shortcut={shortcut}
								onOpenResources={onOpenResources}
								onOpenArgo={onOpenArgo}
							/>
						))}
					</CardContent>
				</Card>
				<Card>
					<CardHeader>
						<CardTitle>Argo CD</CardTitle>
						<CardDescription>
							Application inventory for this cluster.
						</CardDescription>
					</CardHeader>
					<CardContent>
					{argoDetectedQuery.isPending && (
						<div className="inline-flex items-center gap-2 text-xs text-muted-foreground">
							<Spinner className="size-4" />
							Detecting...
						</div>
					)}
					{argoDetectedQuery.data === false && (
						<div className="text-xs text-muted-foreground">Not detected</div>
					)}
					{argoDetectedQuery.data === true && (
						<div className="grid gap-2">
							{argoAppsQuery.isPending && (
								<div className="inline-flex items-center gap-2 text-xs text-muted-foreground">
									<Spinner className="size-4" />
									Loading applications...
								</div>
							)}
							{argoAppsQuery.isError && (
								<Alert variant="destructive">
									<AlertTitle>Failed to load applications</AlertTitle>
									<AlertDescription>
										Application counts are unavailable until the next refresh.
									</AlertDescription>
								</Alert>
							)}
							{argoApps && (
								<>
									<div className="flex items-center justify-between py-2 text-xs">
										<span className="text-muted-foreground">Applications</span>
										<strong>{argoApps.length}</strong>
									</div>
									<Separator />
									<div className="flex items-center justify-between py-2 text-xs">
										<span className="text-muted-foreground">Out of sync</span>
										<strong className="text-amber-300">{argoDrift}</strong>
									</div>
									<Separator />
								</>
							)}
							<Button type="button" variant="outline" onClick={() => onOpenArgo()}>
								<GitBranch data-icon="inline-start" />
								Open Argo CD
							</Button>
						</div>
					)}
					</CardContent>
				</Card>
			</div>

			{resourcesQuery.isError && (
				<Alert variant="destructive">
					<AlertTitle>Failed to refresh workspace resources</AlertTitle>
					<AlertDescription>
						Resource health may be stale until the next refresh succeeds.
					</AlertDescription>
				</Alert>
			)}
		</div>
	);
}

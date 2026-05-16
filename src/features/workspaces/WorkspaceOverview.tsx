import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, Boxes, FolderOpen, GitBranch, Layers } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { fetchWorkspaceResources } from "./query";

interface WorkspaceOverviewProps {
	workspace: SavedWorkspace;
	onOpenResources: (namespace?: string) => void;
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
		<div className="flex min-h-20 flex-col justify-center gap-1 rounded-md border bg-card p-4">
			<span className="text-[0.72rem] font-semibold uppercase text-muted-foreground">
				{label}
			</span>
			<strong className={tone}>{value}</strong>
		</div>
	);
}

function ShortcutButton({
	shortcut,
	onOpenResources,
	onOpenArgo,
}: {
	shortcut: WorkspaceShortcut;
	onOpenResources: (namespace?: string) => void;
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

export function WorkspaceOverview({
	workspace,
	onOpenResources,
	onOpenArgo,
	onOpenLauncher,
}: WorkspaceOverviewProps) {
	const client = useMemo(() => createTauriClient(), []);
	const contextsQuery = useQuery({
		queryKey: ["workspace-overview-contexts"],
		queryFn: () => listKubeContexts(client),
	});
	const contexts = contextsQuery.data ?? [];
	const clusterAvailable =
		contextsQuery.isPending ||
		contexts.some((context) => context.name === workspace.scope.clusterContext);

	const namespacesQuery = useQuery({
		queryKey: ["workspace-overview-namespaces", workspace.scope.clusterContext],
		queryFn: () => listNamespaces(client, workspace.scope.clusterContext),
		enabled: clusterAvailable && !contextsQuery.isPending,
	});
	const namespaces = namespacesQuery.data?.map((namespace) => namespace.name) ?? [];

	const kindsQuery = useQuery({
		queryKey: ["workspace-overview-kinds", workspace.scope.clusterContext],
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

	const resourcesQuery = useQuery({
		queryKey: [
			"workspace-overview-resources",
			workspace.id,
			workspace.updatedAt,
			availableNamespaces.join(","),
		],
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
		queryKey: ["workspace-overview-argo-detect", workspace.scope.clusterContext],
		queryFn: () => detectArgoCD(client, workspace.scope.clusterContext),
		enabled: restoreStatus.clusterAvailable,
	});
	const argoAppsQuery = useQuery({
		queryKey: ["workspace-overview-argo-apps", workspace.scope.clusterContext],
		queryFn: () => listArgoApplications(client, workspace.scope.clusterContext),
		enabled: argoDetectedQuery.data === true,
	});
	const argoApps = argoAppsQuery.data ?? [];
	const argoDrift = argoApps.filter(
		(app) => app.syncStatus && app.syncStatus !== "Synced",
	).length;

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
				<div className="rounded-md border border-amber-500/40 bg-card p-3 text-xs text-amber-200">
					<div className="mb-1 flex items-center gap-2 font-semibold">
						<AlertTriangle className="size-3.5" />
						Unavailable saved scope
					</div>
					{!restoreStatus.clusterAvailable && (
						<div>Context missing: {workspace.scope.clusterContext}</div>
					)}
					{restoreStatus.missingNamespaces.length > 0 && (
						<div>Namespaces: {restoreStatus.missingNamespaces.join(", ")}</div>
					)}
					{restoreStatus.missingKinds.length > 0 && (
						<div>Kinds: {restoreStatus.missingKinds.join(", ")}</div>
					)}
				</div>
			)}

			<div className="grid grid-cols-1 gap-2 md:grid-cols-5">
				<Metric label="Total" value={health.total} />
				<Metric label="Healthy" value={health.healthy} tone="text-emerald-300" />
				<Metric label="Needs attention" value={health.attention} tone="text-amber-300" />
				<Metric label="Degraded" value={health.degraded} tone="text-red-300" />
				<Metric label="Restarted" value={health.restarted} tone="text-sky-300" />
			</div>

			<div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
				<section className="rounded-md border bg-card p-4">
					<div className="mb-3 flex items-center justify-between gap-3">
						<h2 className="text-sm font-semibold">Shortcuts</h2>
						<span className="text-xs text-muted-foreground">
							{workspace.shortcuts.length}
						</span>
					</div>
					<div className="flex flex-wrap gap-2">
						{workspace.shortcuts.map((shortcut) => (
							<ShortcutButton
								key={shortcut.id}
								shortcut={shortcut}
								onOpenResources={onOpenResources}
								onOpenArgo={onOpenArgo}
							/>
						))}
					</div>
				</section>
				<section className="rounded-md border bg-card p-4">
					<h2 className="mb-3 text-sm font-semibold">Argo CD</h2>
					{argoDetectedQuery.isPending && (
						<div className="text-xs text-muted-foreground">Detecting...</div>
					)}
					{argoDetectedQuery.data === false && (
						<div className="text-xs text-muted-foreground">Not detected</div>
					)}
					{argoDetectedQuery.data === true && (
						<div className="grid gap-2">
							<div className="flex items-center justify-between border-b py-2 text-xs">
								<span className="text-muted-foreground">Applications</span>
								<strong>{argoApps.length}</strong>
							</div>
							<div className="flex items-center justify-between border-b py-2 text-xs">
								<span className="text-muted-foreground">Out of sync</span>
								<strong className="text-amber-300">{argoDrift}</strong>
							</div>
							<Button type="button" variant="outline" onClick={() => onOpenArgo()}>
								<GitBranch data-icon="inline-start" />
								Open Argo CD
							</Button>
						</div>
					)}
				</section>
			</div>

			{resourcesQuery.isError && (
				<div className="rounded-md border border-destructive/40 bg-card p-3 text-xs text-destructive">
					Failed to refresh workspace resources.
				</div>
			)}
		</div>
	);
}

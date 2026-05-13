import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import {
	createTauriClient,
	detectArgoCD,
	listArgoApplications,
	listArgoApplicationSets,
	listArgoAppProjects,
} from "../../lib/tauri";
import type {
	ArgoApplicationSummary,
	ArgoApplicationSetSummary,
	ArgoAppProjectSummary,
} from "../../lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Search, X } from "lucide-react";
import { cn } from "@/lib/utils";

interface ArgoCDPanelProps {
	clusterContext: string;
	selectedArgoItem:
		| ArgoApplicationSummary
		| ArgoApplicationSetSummary
		| ArgoAppProjectSummary
		| null;
	onArgoItemSelect: (
		item:
			| ArgoApplicationSummary
			| ArgoApplicationSetSummary
			| ArgoAppProjectSummary,
	) => void;
	selectedArgoKind: string | null;
}

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
const TABLE_CLASS =
	"w-full table-fixed border-collapse text-sm [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-3 [&_td]:py-3 [&_td]:truncate";
const ROW_CLASS = "cursor-pointer transition-colors hover:bg-accent/60";
const SELECTED_ROW_CLASS = "bg-accent";
const EMPTY_PAGE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const STATE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const ERROR_STATE_CLASS = "p-8 text-center text-sm text-destructive";
const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";
const PAGINATION_CLASS = "flex items-center border-t py-2 text-xs text-muted-foreground";

function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string | null | undefined;
	variant?: ChipVariant;
}) {
	if (!value) return null;
	const badgeStyle = CHIP_BADGE_STYLES[variant];
	return (
		<Badge
			variant={badgeStyle.variant}
			className={cn(
				"rounded-full px-2 py-0 text-[0.6875rem] shadow-none",
				badgeStyle.className,
			)}
		>
			{value}
		</Badge>
	);
}

function syncStatusVariant(status: string | null): ChipVariant {
	if (status === "Synced") return "success";
	if (status === "OutOfSync") return "warning";
	if (status === "Unknown") return "neutral";
	return "neutral";
}

function healthStatusVariant(status: string | null): ChipVariant {
	if (status === "Healthy") return "success";
	if (status === "Degraded" || status === "Missing") return "error";
	if (status === "Progressing" || status === "Unknown") return "warning";
	return "neutral";
}

// ── Applications table ──────────────────────────────────────────────────────

function ApplicationsTable({
	apps,
	selectedArgoApp,
	onAppSelect,
}: {
	apps: ArgoApplicationSummary[];
	selectedArgoApp: ArgoApplicationSummary | null;
	onAppSelect: (app: ArgoApplicationSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const filtered = search.trim()
		? apps.filter(
				(app) =>
					app.name.toLowerCase().includes(search.toLowerCase()) ||
					app.project?.toLowerCase().includes(search.toLowerCase()) ||
					app.sourceRepo?.toLowerCase().includes(search.toLowerCase()),
			)
		: apps;

	return (
		<>
			<div className={TOOLBAR_CLASS}>
				<div className="relative min-w-0 flex-1">
					<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						className="h-9 border-slate-700/80 bg-slate-950/45 pl-8 text-sm text-foreground placeholder:text-muted-foreground"
						type="text"
						placeholder="Search by name, project, repo..."
						value={search}
						onChange={(e) => setSearch(e.target.value)}
					/>
				</div>
				{search && (
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => setSearch("")}
					>
						<X className="size-3.5" />
						Clear
					</Button>
				)}
			</div>
			<table className={TABLE_CLASS}>
				<thead>
					<tr>
						<th>Name</th>
						<th>Project</th>
						<th>Sync Status</th>
						<th>Health</th>
						<th>Destination</th>
						<th>Repo</th>
						<th>Revision</th>
						<th>Age</th>
					</tr>
				</thead>
				<tbody>
					{filtered.length === 0 ? (
						<tr>
							<td colSpan={8} className={EMPTY_PAGE_CLASS}>
								No applications found
							</td>
						</tr>
					) : (
						filtered.map((app) => {
							const isSelected =
								selectedArgoApp !== null &&
								app.name === selectedArgoApp.name &&
								app.namespace === selectedArgoApp.namespace;
							return (
								<tr
									key={app.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onAppSelect(app)}
								>
									<td>{app.name}</td>
									<td>{app.project ?? "—"}</td>
									<td>
										<StatusChip
											value={app.syncStatus}
											variant={syncStatusVariant(app.syncStatus)}
										/>
									</td>
									<td>
										<StatusChip
											value={app.healthStatus}
											variant={healthStatusVariant(app.healthStatus)}
										/>
									</td>
									<td>{app.destinationNamespace ?? "—"}</td>
									<td title={app.sourceRepo ?? undefined}>
										{app.sourceRepo?.split("/").pop() ?? "—"}
									</td>
									<td>{app.sourceRevision ?? "—"}</td>
									<td>{app.age}</td>
								</tr>
							);
						})
					)}
				</tbody>
			</table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} applications
				</span>
			</div>
		</>
	);
}

// ── ApplicationSets table ────────────────────────────────────────────────────

function ApplicationSetsTable({
	appsets,
	selectedArgoItem,
	onArgoItemSelect,
}: {
	appsets: ArgoApplicationSetSummary[];
	selectedArgoItem: ArgoApplicationSetSummary | null;
	onArgoItemSelect: (item: ArgoApplicationSetSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const filtered = search.trim()
		? appsets.filter(
				(as) =>
					as.name.toLowerCase().includes(search.toLowerCase()) ||
					as.project?.toLowerCase().includes(search.toLowerCase()),
			)
		: appsets;

	return (
		<>
			<div className={TOOLBAR_CLASS}>
				<div className="relative min-w-0 flex-1">
					<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						className="h-9 border-slate-700/80 bg-slate-950/45 pl-8 text-sm text-foreground placeholder:text-muted-foreground"
						type="text"
						placeholder="Search by name, project..."
						value={search}
						onChange={(e) => setSearch(e.target.value)}
					/>
				</div>
				{search && (
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => setSearch("")}
					>
						<X className="size-3.5" />
						Clear
					</Button>
				)}
			</div>
			<table className={TABLE_CLASS}>
				<thead>
					<tr>
						<th>Name</th>
						<th>Project</th>
						<th>Status</th>
						<th>Sync Status</th>
						<th>Health</th>
						<th>Destination</th>
						<th>Repo</th>
						<th>Revision</th>
						<th>Age</th>
					</tr>
				</thead>
				<tbody>
					{filtered.length === 0 ? (
						<tr>
							<td colSpan={9} className={EMPTY_PAGE_CLASS}>
								No application sets found
							</td>
						</tr>
					) : (
						filtered.map((as) => {
							const isSelected =
								selectedArgoItem !== null &&
								as.name === selectedArgoItem.name &&
								as.namespace === selectedArgoItem.namespace;
							return (
								<tr
									key={as.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onArgoItemSelect(as)}
								>
									<td>{as.name}</td>
									<td>{as.project ?? "—"}</td>
									<td>
										<StatusChip value={as.status} variant="neutral" />
									</td>
									<td>
										<StatusChip
											value={as.syncStatus}
											variant={syncStatusVariant(as.syncStatus)}
										/>
									</td>
									<td>
										<StatusChip
											value={as.healthStatus}
											variant={healthStatusVariant(as.healthStatus)}
										/>
									</td>
									<td>{as.destinationNamespace ?? "—"}</td>
									<td title={as.sourceRepo ?? undefined}>
										{as.sourceRepo?.split("/").pop() ?? "—"}
									</td>
									<td>{as.sourceRevision ?? "—"}</td>
									<td>{as.age}</td>
								</tr>
							);
						})
					)}
				</tbody>
			</table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} application sets
				</span>
			</div>
		</>
	);
}

// ── AppProjects table ────────────────────────────────────────────────────────

function AppProjectsTable({
	projects,
	selectedArgoItem,
	onArgoItemSelect,
}: {
	projects: ArgoAppProjectSummary[];
	selectedArgoItem: ArgoAppProjectSummary | null;
	onArgoItemSelect: (item: ArgoAppProjectSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const filtered = search.trim()
		? projects.filter(
				(p) =>
					p.name.toLowerCase().includes(search.toLowerCase()) ||
					p.description?.toLowerCase().includes(search.toLowerCase()),
			)
		: projects;

	return (
		<>
			<div className={TOOLBAR_CLASS}>
				<div className="relative min-w-0 flex-1">
					<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
					<Input
						className="h-9 border-slate-700/80 bg-slate-950/45 pl-8 text-sm text-foreground placeholder:text-muted-foreground"
						type="text"
						placeholder="Search by name, description..."
						value={search}
						onChange={(e) => setSearch(e.target.value)}
					/>
				</div>
				{search && (
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={() => setSearch("")}
					>
						<X className="size-3.5" />
						Clear
					</Button>
				)}
			</div>
			<table className={TABLE_CLASS}>
				<thead>
					<tr>
						<th>Name</th>
						<th>Description</th>
						<th>Status</th>
						<th>Age</th>
					</tr>
				</thead>
				<tbody>
					{filtered.length === 0 ? (
						<tr>
							<td colSpan={4} className={EMPTY_PAGE_CLASS}>
								No app projects found
							</td>
						</tr>
					) : (
						filtered.map((p) => {
							const isSelected =
								selectedArgoItem !== null &&
								p.name === selectedArgoItem.name &&
								p.namespace === selectedArgoItem.namespace;
							return (
								<tr
									key={p.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onArgoItemSelect(p)}
								>
									<td>{p.name}</td>
									<td>{p.description ?? "—"}</td>
									<td>
										<StatusChip value={p.status} variant="neutral" />
									</td>
									<td>{p.age}</td>
								</tr>
							);
						})
					)}
				</tbody>
			</table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} app projects
				</span>
			</div>
		</>
	);
}

// ── Main ArgoCDPanel ─────────────────────────────────────────────────────────

export function ArgoCDPanel({
	clusterContext,
	selectedArgoItem,
	onArgoItemSelect,
	selectedArgoKind,
}: ArgoCDPanelProps) {
	const client = useMemo(() => createTauriClient(), []);

	const { data: argoDetected, isPending: detectPending } = useQuery({
		queryKey: ["argo-detect", clusterContext],
		queryFn: () => detectArgoCD(client, clusterContext),
		enabled: !!clusterContext,
		staleTime: 60_000,
	});

	const isApps = selectedArgoKind === "Applications";
	const isAppSets = selectedArgoKind === "ApplicationSets";
	const isAppProjects = selectedArgoKind === "AppProjects";

	const {
		data: apps,
		isPending: appsPending,
		isError: appsError,
		error: appsErr,
	} = useQuery({
		queryKey: ["argo-apps", clusterContext],
		queryFn: () => listArgoApplications(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isApps,
		staleTime: 30_000,
	});

	const {
		data: appsets,
		isPending: appsetsPending,
		isError: appsetsError,
		error: appsetsErr,
	} = useQuery({
		queryKey: ["argo-appsets", clusterContext],
		queryFn: () => listArgoApplicationSets(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isAppSets,
		staleTime: 30_000,
	});

	const {
		data: projects,
		isPending: projectsPending,
		isError: projectsError,
		error: projectsErr,
	} = useQuery({
		queryKey: ["argo-appprojects", clusterContext],
		queryFn: () => listArgoAppProjects(client, clusterContext),
		enabled: !!clusterContext && argoDetected === true && isAppProjects,
		staleTime: 30_000,
	});

	if (detectPending) {
		return (
			<div className={STATE_CLASS}>
				<span className="inline-flex items-center gap-2">Checking for Argo CD...</span>
			</div>
		);
	}

	if (argoDetected === false) {
		return (
			<div className={STATE_CLASS}>
				<span>Argo CD not detected in this cluster</span>
			</div>
		);
	}

	// Show empty state when no specific Argo kind selected
	if (!selectedArgoKind) {
		return (
			<div className={STATE_CLASS}>
				<span>Select an Argo CD resource type</span>
			</div>
		);
	}

	// ── Applications ────────────────────────────────────────────────────────

	if (isApps) {
		if (appsPending) {
			return (
				<div className={STATE_CLASS}>
					<span className="inline-flex items-center gap-2">
						Loading Argo CD applications...
					</span>
				</div>
			);
		}
		if (appsError) {
			return (
				<div className={ERROR_STATE_CLASS}>
					<span>
						Error:{" "}
						{appsErr instanceof Error
							? appsErr.message
							: "Failed to load applications"}
					</span>
				</div>
			);
		}
		if (!apps) {
			return (
				<div className={STATE_CLASS}>
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="flex flex-col">
				<ApplicationsTable
					apps={apps}
					selectedArgoApp={selectedArgoItem as ArgoApplicationSummary | null}
					onAppSelect={
						onArgoItemSelect as (app: ArgoApplicationSummary) => void
					}
				/>
			</div>
		);
	}

	// ── ApplicationSets ──────────────────────────────────────────────────────

	if (isAppSets) {
		if (appsetsPending) {
			return (
				<div className={STATE_CLASS}>
					<span className="inline-flex items-center gap-2">
						Loading Argo CD application sets...
					</span>
				</div>
			);
		}
		if (appsetsError) {
			return (
				<div className={ERROR_STATE_CLASS}>
					<span>
						Error:{" "}
						{appsetsErr instanceof Error
							? appsetsErr.message
							: "Failed to load application sets"}
					</span>
				</div>
			);
		}
		if (!appsets) {
			return (
				<div className={STATE_CLASS}>
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="flex flex-col">
				<ApplicationSetsTable
					appsets={appsets}
					selectedArgoItem={selectedArgoItem as ArgoApplicationSetSummary | null}
					onArgoItemSelect={onArgoItemSelect as (item: ArgoApplicationSetSummary) => void}
				/>
			</div>
		);
	}

	// ── AppProjects ───────────────────────────────────────────────────────────

	if (isAppProjects) {
		if (projectsPending) {
			return (
				<div className={STATE_CLASS}>
					<span className="inline-flex items-center gap-2">
						Loading Argo CD app projects...
					</span>
				</div>
			);
		}
		if (projectsError) {
			return (
				<div className={ERROR_STATE_CLASS}>
					<span>
						Error:{" "}
						{projectsErr instanceof Error
							? projectsErr.message
							: "Failed to load app projects"}
					</span>
				</div>
			);
		}
		if (!projects) {
			return (
				<div className={STATE_CLASS}>
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="flex flex-col">
				<AppProjectsTable
					projects={projects}
					selectedArgoItem={selectedArgoItem as ArgoAppProjectSummary | null}
					onArgoItemSelect={onArgoItemSelect as (item: ArgoAppProjectSummary) => void}
				/>
			</div>
		);
	}

	// Fallback — should not reach here
	return (
		<div className={STATE_CLASS}>
			<span>{selectedArgoKind} is not yet supported</span>
		</div>
	);
}

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

function StatusChip({
	value,
	variant = "neutral",
}: {
	value: string | null | undefined;
	variant?: ChipVariant;
}) {
	if (!value) return null;
	return <span className={`chip chip-${variant}`}>{value}</span>;
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
			<div className="resource-list-toolbar">
				<input
					className="resource-search-input"
					type="text"
					placeholder="Search by name, project, repo..."
					value={search}
					onChange={(e) => setSearch(e.target.value)}
				/>
				{search && (
					<button className="clear-filter-btn" onClick={() => setSearch("")}>
						Clear
					</button>
				)}
			</div>
			<table className="resource-table">
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
							<td colSpan={8} className="empty-page-state">
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
									className={`resource-row${isSelected ? " selected" : ""}`}
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
			<div className="table-pagination">
				<span className="pagination-info">
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
			<div className="resource-list-toolbar">
				<input
					className="resource-search-input"
					type="text"
					placeholder="Search by name, project..."
					value={search}
					onChange={(e) => setSearch(e.target.value)}
				/>
				{search && (
					<button className="clear-filter-btn" onClick={() => setSearch("")}>
						Clear
					</button>
				)}
			</div>
			<table className="resource-table">
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
							<td colSpan={9} className="empty-page-state">
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
									className={`resource-row${isSelected ? " selected" : ""}`}
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
			<div className="table-pagination">
				<span className="pagination-info">
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
			<div className="resource-list-toolbar">
				<input
					className="resource-search-input"
					type="text"
					placeholder="Search by name, description..."
					value={search}
					onChange={(e) => setSearch(e.target.value)}
				/>
				{search && (
					<button className="clear-filter-btn" onClick={() => setSearch("")}>
						Clear
					</button>
				)}
			</div>
			<table className="resource-table">
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
							<td colSpan={4} className="empty-page-state">
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
									className={`resource-row${isSelected ? " selected" : ""}`}
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
			<div className="table-pagination">
				<span className="pagination-info">
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
			<div className="resource-list-state">
				<span className="loading-indicator">Checking for Argo CD...</span>
			</div>
		);
	}

	if (argoDetected === false) {
		return (
			<div className="resource-list-state empty-state">
				<span>Argo CD not detected in this cluster</span>
			</div>
		);
	}

	// Show empty state when no specific Argo kind selected
	if (!selectedArgoKind) {
		return (
			<div className="resource-list-state empty-state">
				<span>Select an Argo CD resource type</span>
			</div>
		);
	}

	// ── Applications ────────────────────────────────────────────────────────

	if (isApps) {
		if (appsPending) {
			return (
				<div className="resource-list-state">
					<span className="loading-indicator">
						Loading Argo CD applications...
					</span>
				</div>
			);
		}
		if (appsError) {
			return (
				<div className="resource-list-state error-state">
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
				<div className="resource-list-state empty-state">
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="argo-panel">
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
				<div className="resource-list-state">
					<span className="loading-indicator">
						Loading Argo CD application sets...
					</span>
				</div>
			);
		}
		if (appsetsError) {
			return (
				<div className="resource-list-state error-state">
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
				<div className="resource-list-state empty-state">
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="argo-panel">
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
				<div className="resource-list-state">
					<span className="loading-indicator">
						Loading Argo CD app projects...
					</span>
				</div>
			);
		}
		if (projectsError) {
			return (
				<div className="resource-list-state error-state">
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
				<div className="resource-list-state empty-state">
					<span>Loading…</span>
				</div>
			);
		}
		return (
			<div className="argo-panel">
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
		<div className="resource-list-state empty-state">
			<span>{selectedArgoKind} is not yet supported</span>
		</div>
	);
}

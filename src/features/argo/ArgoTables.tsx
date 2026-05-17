import { useState } from "react";
import { Search, X } from "lucide-react";
import { TimestampText } from "@/components/TimestampText";
import { Button } from "@/components/ui/button";
import {
	InputGroup,
	InputGroupAddon,
	InputGroupInput,
	InputGroupText,
} from "@/components/ui/input-group";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import type {
	ArgoApplicationSetSummary,
	ArgoApplicationSummary,
	ArgoAppProjectSummary,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { healthStatusVariant, StatusChip, syncStatusVariant } from "./status";

const TABLE_CLASS =
	"w-full table-fixed border-collapse text-sm [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-3 [&_td]:py-3 [&_td]:truncate";
const ROW_CLASS = "cursor-pointer transition-colors hover:bg-accent/60";
const SELECTED_ROW_CLASS = "bg-accent";
const EMPTY_PAGE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";
const PAGINATION_CLASS =
	"flex items-center border-t py-2 text-xs text-muted-foreground";

function ArgoSearchToolbar({
	search,
	placeholder,
	onSearchChange,
}: {
	search: string;
	placeholder: string;
	onSearchChange: (search: string) => void;
}) {
	return (
		<div className={TOOLBAR_CLASS}>
			<InputGroup className="h-9 min-w-0 flex-1 border-slate-700/80 bg-slate-950/45">
				<InputGroupAddon align="inline-start">
					<InputGroupText>
						<Search className="size-4" />
					</InputGroupText>
				</InputGroupAddon>
				<InputGroupInput
					className="h-8 text-sm text-foreground placeholder:text-muted-foreground"
					type="text"
					placeholder={placeholder}
					value={search}
					onChange={(event) => onSearchChange(event.target.value)}
				/>
			</InputGroup>
			{search && (
				<Button
					type="button"
					variant="outline"
					size="sm"
					onClick={() => onSearchChange("")}
				>
					<X className="size-3.5" />
					Clear
				</Button>
			)}
		</div>
	);
}

export function ApplicationsTable({
	apps,
	selectedArgoApp,
	onAppSelect,
}: {
	apps: ArgoApplicationSummary[];
	selectedArgoApp: ArgoApplicationSummary | null;
	onAppSelect: (app: ArgoApplicationSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const searchTerm = search.trim().toLowerCase();
	const filtered = searchTerm
		? apps.filter(
				(app) =>
					app.name.toLowerCase().includes(searchTerm) ||
					app.project?.toLowerCase().includes(searchTerm) ||
					app.sourceRepo?.toLowerCase().includes(searchTerm),
			)
		: apps;

	return (
		<>
			<ArgoSearchToolbar
				search={search}
				placeholder="Search by name, project, repo..."
				onSearchChange={setSearch}
			/>
			<Table className={TABLE_CLASS}>
				<TableHeader>
					<TableRow>
						<TableHead>Name</TableHead>
						<TableHead>Project</TableHead>
						<TableHead>Sync Status</TableHead>
						<TableHead>Health</TableHead>
						<TableHead>Destination</TableHead>
						<TableHead>Repo</TableHead>
						<TableHead>Revision</TableHead>
						<TableHead>Age</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{filtered.length === 0 ? (
						<TableRow>
							<TableCell colSpan={8} className={EMPTY_PAGE_CLASS}>
								No applications found
							</TableCell>
						</TableRow>
					) : (
						filtered.map((app) => {
							const isSelected =
								selectedArgoApp !== null &&
								app.name === selectedArgoApp.name &&
								app.namespace === selectedArgoApp.namespace;
							return (
								<TableRow
									key={app.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onAppSelect(app)}
								>
									<TableCell>{app.name}</TableCell>
									<TableCell>{app.project ?? "—"}</TableCell>
									<TableCell>
										<StatusChip
											value={app.syncStatus}
											variant={syncStatusVariant(app.syncStatus)}
										/>
									</TableCell>
									<TableCell>
										<StatusChip
											value={app.healthStatus}
											variant={healthStatusVariant(app.healthStatus)}
										/>
									</TableCell>
									<TableCell>{app.destinationNamespace ?? "—"}</TableCell>
									<TableCell title={app.sourceRepo ?? undefined}>
										{app.sourceRepo?.split("/").pop() ?? "—"}
									</TableCell>
									<TableCell>{app.sourceRevision ?? "—"}</TableCell>
									<TableCell>
										<TimestampText
											relative={app.age}
											exact={app.createdAt}
											className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</TableCell>
								</TableRow>
							);
						})
					)}
				</TableBody>
			</Table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} applications
				</span>
			</div>
		</>
	);
}

export function ApplicationSetsTable({
	appsets,
	selectedArgoItem,
	onArgoItemSelect,
}: {
	appsets: ArgoApplicationSetSummary[];
	selectedArgoItem: ArgoApplicationSetSummary | null;
	onArgoItemSelect: (item: ArgoApplicationSetSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const searchTerm = search.trim().toLowerCase();
	const filtered = searchTerm
		? appsets.filter(
				(appset) =>
					appset.name.toLowerCase().includes(searchTerm) ||
					appset.project?.toLowerCase().includes(searchTerm),
			)
		: appsets;

	return (
		<>
			<ArgoSearchToolbar
				search={search}
				placeholder="Search by name, project..."
				onSearchChange={setSearch}
			/>
			<Table className={TABLE_CLASS}>
				<TableHeader>
					<TableRow>
						<TableHead>Name</TableHead>
						<TableHead>Project</TableHead>
						<TableHead>Status</TableHead>
						<TableHead>Sync Status</TableHead>
						<TableHead>Health</TableHead>
						<TableHead>Destination</TableHead>
						<TableHead>Repo</TableHead>
						<TableHead>Revision</TableHead>
						<TableHead>Age</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{filtered.length === 0 ? (
						<TableRow>
							<TableCell colSpan={9} className={EMPTY_PAGE_CLASS}>
								No application sets found
							</TableCell>
						</TableRow>
					) : (
						filtered.map((appset) => {
							const isSelected =
								selectedArgoItem !== null &&
								appset.name === selectedArgoItem.name &&
								appset.namespace === selectedArgoItem.namespace;
							return (
								<TableRow
									key={appset.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onArgoItemSelect(appset)}
								>
									<TableCell>{appset.name}</TableCell>
									<TableCell>{appset.project ?? "—"}</TableCell>
									<TableCell>
										<StatusChip value={appset.status} variant="neutral" />
									</TableCell>
									<TableCell>
										<StatusChip
											value={appset.syncStatus}
											variant={syncStatusVariant(appset.syncStatus)}
										/>
									</TableCell>
									<TableCell>
										<StatusChip
											value={appset.healthStatus}
											variant={healthStatusVariant(appset.healthStatus)}
										/>
									</TableCell>
									<TableCell>{appset.destinationNamespace ?? "—"}</TableCell>
									<TableCell title={appset.sourceRepo ?? undefined}>
										{appset.sourceRepo?.split("/").pop() ?? "—"}
									</TableCell>
									<TableCell>{appset.sourceRevision ?? "—"}</TableCell>
									<TableCell>
										<TimestampText
											relative={appset.age}
											exact={appset.createdAt}
											className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</TableCell>
								</TableRow>
							);
						})
					)}
				</TableBody>
			</Table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} application sets
				</span>
			</div>
		</>
	);
}

export function AppProjectsTable({
	projects,
	selectedArgoItem,
	onArgoItemSelect,
}: {
	projects: ArgoAppProjectSummary[];
	selectedArgoItem: ArgoAppProjectSummary | null;
	onArgoItemSelect: (item: ArgoAppProjectSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const searchTerm = search.trim().toLowerCase();
	const filtered = searchTerm
		? projects.filter(
				(project) =>
					project.name.toLowerCase().includes(searchTerm) ||
					project.description?.toLowerCase().includes(searchTerm),
			)
		: projects;

	return (
		<>
			<ArgoSearchToolbar
				search={search}
				placeholder="Search by name, description..."
				onSearchChange={setSearch}
			/>
			<Table className={TABLE_CLASS}>
				<TableHeader>
					<TableRow>
						<TableHead>Name</TableHead>
						<TableHead>Description</TableHead>
						<TableHead>Status</TableHead>
						<TableHead>Age</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{filtered.length === 0 ? (
						<TableRow>
							<TableCell colSpan={4} className={EMPTY_PAGE_CLASS}>
								No app projects found
							</TableCell>
						</TableRow>
					) : (
						filtered.map((project) => {
							const isSelected =
								selectedArgoItem !== null &&
								project.name === selectedArgoItem.name &&
								project.namespace === selectedArgoItem.namespace;
							return (
								<TableRow
									key={project.name}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onArgoItemSelect(project)}
								>
									<TableCell>{project.name}</TableCell>
									<TableCell>{project.description ?? "—"}</TableCell>
									<TableCell>
										<StatusChip value={project.status} variant="neutral" />
									</TableCell>
									<TableCell>
										<TimestampText
											relative={project.age}
											exact={project.createdAt}
											className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</TableCell>
								</TableRow>
							);
						})
					)}
				</TableBody>
			</Table>
			<div className={PAGINATION_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} app projects
				</span>
			</div>
		</>
	);
}

import { useQuery } from "@tanstack/react-query";
import { useMemo, useState, type KeyboardEvent } from "react";
import { Search, X } from "lucide-react";
import {
	Alert,
	AlertDescription,
	AlertTitle,
} from "@/components/ui/alert";
import {
	Empty,
	EmptyDescription,
	EmptyHeader,
	EmptyTitle,
} from "@/components/ui/empty";
import { Button } from "@/components/ui/button";
import {
	InputGroup,
	InputGroupAddon,
	InputGroupInput,
	InputGroupText,
} from "@/components/ui/input-group";
import { Spinner } from "@/components/ui/spinner";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { StatusBadge } from "@/components/StatusBadge";
import { TimestampText } from "@/components/TimestampText";
import { queryKeys } from "@/lib/queryKeys";
import { createTauriClient, listHelmReleases } from "@/lib/tauri";
import type { HelmReleaseSummary } from "@/lib/types";
import { cn } from "@/lib/utils";

interface HelmPanelProps {
	clusterContext: string;
	selectedRelease: HelmReleaseSummary | null;
	onReleaseSelect: (release: HelmReleaseSummary) => void;
}

const STATE_CLASS =
	"flex min-h-64 items-center justify-center p-8 text-center text-sm text-muted-foreground";
const TABLE_CLASS =
	"w-full table-fixed border-collapse text-sm [&_th]:border-b-2 [&_th]:px-3 [&_th]:py-3 [&_th]:text-left [&_th]:text-xs [&_th]:font-semibold [&_th]:uppercase [&_th]:text-muted-foreground [&_td]:whitespace-nowrap [&_td]:border-b [&_td]:px-3 [&_td]:py-3 [&_td]:truncate";
const ROW_CLASS = "cursor-pointer transition-colors hover:bg-accent/60";
const SELECTED_ROW_CLASS = "bg-accent";
const EMPTY_PAGE_CLASS = "p-8 text-center text-sm text-muted-foreground";
const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";
const FOOTER_CLASS =
	"flex items-center border-t py-2 text-xs text-muted-foreground";

function LoadingState() {
	return (
		<div className={STATE_CLASS}>
			<span className="inline-flex items-center gap-2">
				<Spinner className="size-4" />
				Loading Helm releases...
			</span>
		</div>
	);
}

function ErrorState({ error }: { error: unknown }) {
	return (
		<div className="p-4">
			<Alert variant="destructive">
				<AlertTitle>Failed to load Helm releases</AlertTitle>
				<AlertDescription>
					{error instanceof Error ? error.message : "Failed to load Helm releases"}
				</AlertDescription>
			</Alert>
		</div>
	);
}

function EmptyState() {
	return (
		<Empty className="min-h-64 border-0">
			<EmptyHeader>
				<EmptyTitle>No Helm releases found</EmptyTitle>
				<EmptyDescription>
					This cluster does not expose Helm v3 release storage objects.
				</EmptyDescription>
			</EmptyHeader>
		</Empty>
	);
}

function releaseKey(release: HelmReleaseSummary): string {
	return `${release.namespace}:${release.name}:${release.storageKind}:${release.storageName}`;
}

function statusTone(status: string | undefined) {
	if (status === "deployed" || status === "superseded") return "success";
	if (status === "failed" || status === "uninstalled") return "error";
	if (status === "pending-install" || status === "pending-upgrade") return "warning";
	return "neutral";
}

function handleRowActivation(
	event: KeyboardEvent<HTMLTableRowElement>,
	release: HelmReleaseSummary,
	onSelect: (release: HelmReleaseSummary) => void,
) {
	if (event.key !== "Enter" && event.key !== " ") return;
	event.preventDefault();
	onSelect(release);
}

function HelmSearchToolbar({
	search,
	onSearchChange,
}: {
	search: string;
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
					placeholder="Search by release, namespace, chart, app version..."
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

function HelmTable({
	releases,
	selectedRelease,
	onReleaseSelect,
}: {
	releases: HelmReleaseSummary[];
	selectedRelease: HelmReleaseSummary | null;
	onReleaseSelect: (release: HelmReleaseSummary) => void;
}) {
	const [search, setSearch] = useState("");
	const searchTerm = search.trim().toLowerCase();
	const filtered = searchTerm
		? releases.filter(
				(release) =>
					release.name.toLowerCase().includes(searchTerm) ||
					release.namespace.toLowerCase().includes(searchTerm) ||
					release.chart?.toLowerCase().includes(searchTerm) ||
					release.appVersion?.toLowerCase().includes(searchTerm),
			)
		: releases;

	return (
		<>
			<HelmSearchToolbar search={search} onSearchChange={setSearch} />
			<Table className={TABLE_CLASS}>
				<TableHeader>
					<TableRow>
						<TableHead>Release</TableHead>
						<TableHead>Namespace</TableHead>
						<TableHead>Chart</TableHead>
						<TableHead>App Version</TableHead>
						<TableHead>Revision</TableHead>
						<TableHead>Status</TableHead>
						<TableHead>Storage</TableHead>
						<TableHead>Updated</TableHead>
					</TableRow>
				</TableHeader>
				<TableBody>
					{filtered.length === 0 ? (
						<TableRow>
							<TableCell colSpan={8} className={EMPTY_PAGE_CLASS}>
								No Helm releases found
							</TableCell>
						</TableRow>
					) : (
						filtered.map((release) => {
							const isSelected =
								selectedRelease !== null &&
								releaseKey(release) === releaseKey(selectedRelease);
							return (
								<TableRow
									key={releaseKey(release)}
									className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
									onClick={() => onReleaseSelect(release)}
									onKeyDown={(event) =>
										handleRowActivation(event, release, onReleaseSelect)
									}
									tabIndex={0}
									role="button"
									aria-selected={isSelected}
								>
									<TableCell>{release.name}</TableCell>
									<TableCell>{release.namespace}</TableCell>
									<TableCell>{release.chart ?? "-"}</TableCell>
									<TableCell>{release.appVersion ?? "-"}</TableCell>
									<TableCell>{release.revision ?? "-"}</TableCell>
									<TableCell>
										{release.status ? (
											<StatusBadge tone={statusTone(release.status)}>
												{release.status}
											</StatusBadge>
										) : (
											"-"
										)}
									</TableCell>
									<TableCell>{release.storageKind}</TableCell>
									<TableCell>
										<TimestampText
											relative={release.age}
											exact={release.updatedAt ?? release.createdAt}
											className="block min-w-0 truncate outline-none focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring/50"
										/>
									</TableCell>
								</TableRow>
							);
						})
					)}
				</TableBody>
			</Table>
			<div className={FOOTER_CLASS}>
				<span>
					{filtered.length} {search ? "filtered" : "total"} Helm releases
				</span>
			</div>
		</>
	);
}

export function HelmPanel({
	clusterContext,
	selectedRelease,
	onReleaseSelect,
}: HelmPanelProps) {
	const client = useMemo(() => createTauriClient(), []);
	const {
		data: releases,
		isPending,
		isError,
		error,
	} = useQuery({
		queryKey: queryKeys.helmReleases(clusterContext),
		queryFn: () => listHelmReleases(client, clusterContext),
		enabled: !!clusterContext,
		staleTime: 30_000,
	});

	if (isPending) return <LoadingState />;
	if (isError) return <ErrorState error={error} />;
	if (!releases) return <LoadingState />;
	if (releases.length === 0) return <EmptyState />;

	return (
		<div className="flex flex-col">
			<HelmTable
				releases={releases}
				selectedRelease={selectedRelease}
				onReleaseSelect={onReleaseSelect}
			/>
		</div>
	);
}

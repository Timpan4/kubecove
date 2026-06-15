import { lazy, Suspense, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Table as TanStackTable } from "@tanstack/react-table";
import {
	GitBranch,
	PanelRightClose,
	PanelRightOpen,
	Table2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyMode,
	TopologyNode,
} from "@/lib/types";
import { cn } from "@/lib/utils";
import { ResourcePagination } from "./pagination";
import { ResourceTable } from "./ResourceTable";

const LazyOwnershipMap = lazy(() =>
	import("./OwnershipMap").then((module) => ({ default: module.OwnershipMap })),
);

interface ResourceMapTableLayoutProps {
	topology: ResourceTopology | undefined;
	topologyLoading: boolean;
	topologyError: boolean;
	topologyErr: unknown;
	selectedTopologyNodeId: string | null;
	hasDeferredTopologySelection: boolean;
	topologyFitViewKey: string;
	topologyMode: TopologyMode;
	onTopologyModeChange: (mode: TopologyMode) => void;
	mapPanelOpen: boolean;
	onMapPanelOpenChange: (open: boolean) => void;
	onTopologyNodeSelect: (
		node: TopologyNode,
		resource: ResourceSummary | null,
	) => void;
	table: TanStackTable<ResourceSummary>;
	groupedByGitOps: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
	selectedResourceIdentityKey: string | null;
	onToggleGroup: (key: string) => void;
	onSelectedResourceKeyChange: (key: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
	totalRows: number;
	search: string;
	pageIndex: number;
	pageCount: number;
	onPageChange: Dispatch<SetStateAction<number>>;
	realtime: {
		status: string;
		message: string;
		error: string | null;
	};
}

function CollapsedRail({
	icon,
	label,
	detail,
	ariaLabel,
	onOpen,
}: {
	icon: "map" | "table";
	label: string;
	detail?: string;
	ariaLabel: string;
	onOpen: () => void;
}) {
	const Icon = icon === "map" ? GitBranch : Table2;
	return (
		<aside className="flex h-full min-h-[400px] w-12 shrink-0 flex-col items-center overflow-hidden rounded-md border bg-card/60">
			<div className="flex w-full justify-center border-b p-2">
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="size-7"
					onClick={onOpen}
					aria-label={ariaLabel}
				>
					<PanelRightOpen />
				</Button>
			</div>
			<button
				type="button"
				className="flex min-h-0 flex-1 flex-col items-center gap-2 px-2 py-3 text-muted-foreground hover:text-foreground"
				onClick={onOpen}
				aria-label={ariaLabel}
			>
				<Icon className="size-4 shrink-0" />
				<span className="[writing-mode:vertical-rl] text-xs font-semibold">
					{label}
				</span>
				{detail ? (
					<span className="[writing-mode:vertical-rl] text-[0.6875rem]">
						{detail}
					</span>
				) : null}
			</button>
		</aside>
	);
}

export function ResourceMapTableLayout({
	topology,
	topologyLoading,
	topologyError,
	topologyErr,
	selectedTopologyNodeId,
	hasDeferredTopologySelection,
	topologyFitViewKey,
	topologyMode,
	onTopologyModeChange,
	mapPanelOpen,
	onMapPanelOpenChange,
	onTopologyNodeSelect,
	table,
	groupedByGitOps,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
	selectedResourceIdentityKey,
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
	totalRows,
	search,
	pageIndex,
	pageCount,
	onPageChange,
	realtime,
}: ResourceMapTableLayoutProps) {
	const [tablePanelOpen, setTablePanelOpen] = useState(true);
	const hasActiveSelection =
		hasDeferredTopologySelection ||
		Boolean(selectedResourceIdentityKey ?? selectedResourceKey);
	const mapHeightClassName = "h-full min-h-0";
	const tablePanelClassName = cn(
		"flex h-full min-h-[400px] min-w-0 flex-col overflow-hidden rounded-md border bg-card/60",
	);
	const mapPanelClassName = "h-full min-h-[400px] min-w-0";
	const contentGridClassName = cn(
		"grid min-h-0 min-w-0 flex-1 gap-3",
		mapPanelOpen && tablePanelOpen
			? hasActiveSelection
				? "grid-cols-1 grid-rows-[minmax(400px,1fr)_minmax(400px,1fr)]"
				: "xl:grid-cols-[minmax(420px,0.4fr)_minmax(620px,0.6fr)]"
			: mapPanelOpen
				? "grid-cols-[minmax(0,1fr)_3rem]"
				: tablePanelOpen
					? "grid-cols-[3rem_minmax(620px,1fr)]"
					: "grid-cols-[3rem_3rem]",
	);
	const handleMapPanelToggle = () => {
		onMapPanelOpenChange(!mapPanelOpen);
	};
	const handleTablePanelToggle = () => {
		setTablePanelOpen((open) => !open);
	};

	return (
		<div className="flex min-h-0 flex-1 flex-col gap-3">
			<div className="flex items-center gap-2 text-xs text-muted-foreground">
				<Badge variant={realtime.status === "error" ? "destructive" : "outline"}>
					Realtime: {realtime.status}
				</Badge>
				<span className="truncate">{realtime.error ?? realtime.message}</span>
			</div>
			<div className={contentGridClassName}>
				{mapPanelOpen ? (
					<div className={mapPanelClassName}>
						<Suspense
							fallback={
								<div
									className={cn(
										mapHeightClassName,
										"rounded-md border bg-card/60",
									)}
								/>
							}
						>
							<LazyOwnershipMap
								topology={topology}
								isLoading={topologyLoading}
								isError={topologyError}
								error={topologyErr}
								selectedNodeId={selectedTopologyNodeId}
								fitViewKey={topologyFitViewKey}
								mode={topologyMode}
								heightClassName={mapHeightClassName}
								onModeChange={onTopologyModeChange}
								onMapToggle={handleMapPanelToggle}
								onNodeSelect={onTopologyNodeSelect}
							/>
						</Suspense>
					</div>
				) : (
					<CollapsedRail
						icon="map"
						label="Map"
						ariaLabel="Show ownership map"
						onOpen={handleMapPanelToggle}
					/>
				)}
				{tablePanelOpen ? (
					<aside className={tablePanelClassName}>
						<div className="flex items-center justify-between gap-2 border-b px-3 py-2">
							<div className="min-w-0">
								<div className="flex items-center gap-2 text-xs font-semibold text-foreground">
									<Table2 className="size-4" />
									Resource Table
								</div>
								<div className="truncate text-[0.6875rem] text-muted-foreground">
									{totalRows} resources · page {pageIndex + 1} of {pageCount}
								</div>
							</div>
							<Button
								type="button"
								variant="ghost"
								size="icon"
								className="size-7"
								onClick={handleTablePanelToggle}
								aria-label="Collapse resource table"
							>
								<PanelRightClose />
							</Button>
						</div>
						<div className="min-h-0 flex-1 overflow-hidden">
							<ResourceTable
								table={table}
								groupedByGitOps={groupedByGitOps}
								pageGroups={pageGroups}
								pageTypeGroups={pageTypeGroups}
								collapsedGroups={collapsedGroups}
								selectedResourceKey={selectedResourceKey}
								selectedResourceIdentityKey={selectedResourceIdentityKey}
								onToggleGroup={onToggleGroup}
								onSelectedResourceKeyChange={onSelectedResourceKeyChange}
								onResourceSelect={onResourceSelect}
							/>
						</div>
						<div className="border-t px-3 py-2">
							<ResourcePagination
								totalRows={totalRows}
								search={search}
								pageIndex={pageIndex}
								pageCount={pageCount}
								onPageChange={onPageChange}
							/>
						</div>
					</aside>
				) : (
					<CollapsedRail
						icon="table"
						label="Table"
						detail={`${totalRows} rows`}
						ariaLabel="Show resource table"
						onOpen={handleTablePanelToggle}
					/>
				)}
			</div>
		</div>
	);
}

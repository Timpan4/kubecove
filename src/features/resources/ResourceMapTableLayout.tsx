import { lazy, Suspense, useEffect, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { Table as TanStackTable } from "@tanstack/react-table";
import { GitBranch, PanelRightClose, PanelRightOpen, Table2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ResourceSummary, ResourceTopology, TopologyNode } from "@/lib/types";
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
	topologyFitViewKey: string;
	mapPanelOpen: boolean;
	onMapPanelOpenChange: (open: boolean) => void;
	onTopologyNodeSelect: (
		node: TopologyNode,
		resource: ResourceSummary | null,
	) => void;
	table: TanStackTable<ResourceSummary>;
	groupedByArgo: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
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

export function ResourceMapTableLayout({
	topology,
	topologyLoading,
	topologyError,
	topologyErr,
	selectedTopologyNodeId,
	topologyFitViewKey,
	mapPanelOpen,
	onMapPanelOpenChange,
	onTopologyNodeSelect,
	table,
	groupedByArgo,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
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
	useEffect(() => {
		if (!mapPanelOpen) setTablePanelOpen(true);
	}, [mapPanelOpen]);

	const hasActiveSelection = Boolean(
		selectedResourceKey || selectedTopologyNodeId,
	);
	const mapHeightClassName =
		tablePanelOpen && hasActiveSelection ? "h-[360px]" : "h-[640px]";
	const contentGridClassName = mapPanelOpen && tablePanelOpen
		? cn(
				"grid min-w-0 gap-3",
				hasActiveSelection
					? "grid-cols-1"
					: "xl:grid-cols-[minmax(620px,1fr)_minmax(420px,0.82fr)]",
			)
		: "grid min-w-0 gap-3";
	const tablePanelClassName = cn(
		"flex min-w-0 flex-col overflow-hidden rounded-md border bg-card/60",
		hasActiveSelection ? "h-[560px]" : "h-[640px]",
	);
	const handleMapPanelToggle = () => {
		const nextOpen = !mapPanelOpen;
		onMapPanelOpenChange(nextOpen);
		if (!nextOpen) setTablePanelOpen(true);
	};
	const handleTablePanelToggle = () => {
		if (!mapPanelOpen) return;
		setTablePanelOpen((open) => !open);
	};

	return (
		<>
			<div className="flex items-center justify-between gap-2">
				<div className="inline-flex items-center gap-2 text-xs text-muted-foreground">
					<span className="inline-flex items-center gap-1.5 font-medium text-foreground">
						<GitBranch data-icon="inline-start" />
						Map
					</span>
					<span>primary read-only incident view</span>
				</div>
				<div className="flex items-center gap-2">
					<Button
						type="button"
						variant={mapPanelOpen ? "secondary" : "outline"}
						size="sm"
						onClick={handleMapPanelToggle}
						aria-pressed={mapPanelOpen}
					>
						<GitBranch data-icon="inline-start" />
						Map
					</Button>
					<Button
						type="button"
						variant="outline"
						size="sm"
						onClick={handleTablePanelToggle}
						aria-pressed={tablePanelOpen}
						disabled={!mapPanelOpen}
					>
						{tablePanelOpen ? (
							<PanelRightClose data-icon="inline-start" />
						) : (
							<PanelRightOpen data-icon="inline-start" />
						)}
						{tablePanelOpen ? "Hide table" : "Show table"}
					</Button>
				</div>
			</div>
			<div className="flex items-center gap-2 text-xs text-muted-foreground">
				<Badge variant={realtime.status === "error" ? "destructive" : "outline"}>
					Realtime: {realtime.status}
				</Badge>
				<span className="truncate">{realtime.error ?? realtime.message}</span>
			</div>
			<div className={contentGridClassName}>
				{mapPanelOpen && (
					<div className="min-w-0">
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
								heightClassName={mapHeightClassName}
								onNodeSelect={onTopologyNodeSelect}
							/>
						</Suspense>
					</div>
				)}
				{tablePanelOpen && (
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
								aria-label="Hide resource table"
								disabled={!mapPanelOpen}
							>
								<PanelRightClose className="size-4" />
							</Button>
						</div>
						<div className="min-h-0 flex-1 overflow-auto">
							<ResourceTable
								table={table}
								groupedByArgo={groupedByArgo}
								pageGroups={pageGroups}
								pageTypeGroups={pageTypeGroups}
								collapsedGroups={collapsedGroups}
								selectedResourceKey={selectedResourceKey}
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
				)}
			</div>
		</>
	);
}

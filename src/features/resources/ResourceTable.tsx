import { Fragment, type KeyboardEvent } from "react";
import {
	flexRender,
	type Row,
	type Table as TanStackTable,
} from "@tanstack/react-table";
import { ChevronDown, ChevronRight } from "lucide-react";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { diagnosticLog } from "@/lib/diagnostics";
import {
	getResourceGroupVisual,
	getResourceKindVisual,
} from "@/lib/resource-visuals";
import type { ResourceSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { columns } from "./columns";
import {
	EMPTY_PAGE_CLASS,
	ROW_CLASS,
	SELECTED_ROW_CLASS,
	TABLE_CLASS,
} from "./constants";
import {
	formatResourceGroupLabel,
	formatResourceTypeGroupLabel,
	resourceIdentityKey,
	resourceSelectionKey,
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
} from "./helpers";

interface ResourceTableProps {
	table: TanStackTable<ResourceSummary>;
	groupedByArgo: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
	selectedResourceIdentityKey: string | null;
	onToggleGroup: (key: string) => void;
	onSelectedResourceKeyChange: (key: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function sortAriaValue(sortState: false | "asc" | "desc") {
	if (sortState === "asc") return "ascending";
	if (sortState === "desc") return "descending";
	return "none";
}

function isActivationKey(event: KeyboardEvent) {
	return event.key === "Enter" || event.key === " ";
}

export function ResourceTable({
	table,
	groupedByArgo,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
	selectedResourceIdentityKey,
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
}: ResourceTableProps) {
	const rowModel = table.getRowModel();

	return (
		<Table className={TABLE_CLASS}>
			<TableHeader>
				{table.getHeaderGroups().map((headerGroup) => (
					<TableRow key={headerGroup.id}>
						{headerGroup.headers.map((header) => {
							const canSort = header.column.getCanSort();
							const toggleSorting = header.column.getToggleSortingHandler();
							const sortState = header.column.getIsSorted();
							return (
								<TableHead
									key={header.id}
									onClick={canSort ? toggleSorting : undefined}
									onKeyDown={
										canSort
											? (event) => {
													if (!isActivationKey(event)) return;
													event.preventDefault();
													toggleSorting?.(event);
												}
											: undefined
									}
									tabIndex={canSort ? 0 : undefined}
									aria-sort={canSort ? sortAriaValue(sortState) : undefined}
									className={canSort ? "cursor-pointer" : "cursor-default"}
								>
									{flexRender(
										header.column.columnDef.header,
										header.getContext(),
									)}
									{sortState === "asc"
										? " ↑"
										: sortState === "desc"
											? " ↓"
											: ""}
								</TableHead>
							);
						})}
					</TableRow>
				))}
			</TableHeader>
			<TableBody>
				{rowModel.rows.length === 0 ? (
					<TableRow>
						<TableCell colSpan={columns.length} className={EMPTY_PAGE_CLASS}>
							No resources match your filter
						</TableCell>
					</TableRow>
				) : (
					rowModel.rows.map((row, index) => (
						<ResourceTableRow
							key={row.id}
							row={row}
							index={index}
							previous={index > 0 ? rowModel.rows[index - 1]?.original : null}
							groupedByArgo={groupedByArgo}
							pageGroups={pageGroups}
							pageTypeGroups={pageTypeGroups}
							collapsedGroups={collapsedGroups}
							selectedResourceKey={selectedResourceKey}
							selectedResourceIdentityKey={selectedResourceIdentityKey}
							onToggleGroup={onToggleGroup}
							onSelectedResourceKeyChange={onSelectedResourceKeyChange}
							onResourceSelect={onResourceSelect}
						/>
					))
				)}
			</TableBody>
		</Table>
	);
}

interface ResourceTableRowProps {
	row: Row<ResourceSummary>;
	index: number;
	previous: ResourceSummary | null;
	groupedByArgo: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
	selectedResourceIdentityKey: string | null;
	onToggleGroup: (key: string) => void;
	onSelectedResourceKeyChange: (key: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function ResourceTableRow({
	row,
	index,
	previous,
	groupedByArgo,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
	selectedResourceIdentityKey,
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
}: ResourceTableRowProps) {
	const resourceKey = resourceSelectionKey(row.original);
	const identityKey = resourceIdentityKey(row.original);
	const isSelected =
		selectedResourceKey === resourceKey ||
		selectedResourceIdentityKey === identityKey;
	const label = formatResourceGroupLabel(row.original);
	const typeLabel = formatResourceTypeGroupLabel(row.original);
	const typeKey = `${label}::${typeLabel}`;
	const appCollapseKey = resourceGroupCollapseKey(row.original);
	const typeCollapseKey = resourceTypeGroupCollapseKey(row.original);
	const appCollapsed = collapsedGroups.has(appCollapseKey);
	const typeCollapsed = collapsedGroups.has(typeCollapseKey);
	const showGroupHeader =
		groupedByArgo &&
		(!previous || formatResourceGroupLabel(previous) !== label);
	const showTypeGroupHeader =
		groupedByArgo &&
		(!previous ||
			formatResourceGroupLabel(previous) !== label ||
			formatResourceTypeGroupLabel(previous) !== typeLabel);
	const hideResourceRow = groupedByArgo && (appCollapsed || typeCollapsed);
	const selectResource = () => {
		diagnosticLog("resources.row.click", {
			key: resourceKey,
			alreadySelected: isSelected,
			rowIndex: index,
		});
		onSelectedResourceKeyChange(resourceKey);
		onResourceSelect(row.original);
	};

	return (
		<Fragment>
			{showGroupHeader && (
				<ResourceGroupHeader
					label={label}
					count={pageGroups.get(label) ?? 0}
					collapsed={appCollapsed}
					onToggle={() => onToggleGroup(appCollapseKey)}
				/>
			)}
			{showTypeGroupHeader && !appCollapsed && (
				<ResourceTypeGroupHeader
					label={typeLabel}
					kind={row.original.kind}
					count={pageTypeGroups.get(typeKey) ?? 0}
					collapsed={typeCollapsed}
					onToggle={() => onToggleGroup(typeCollapseKey)}
				/>
			)}
			{!hideResourceRow && (
				<TableRow
					className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
					onClick={selectResource}
					onKeyDown={(event) => {
						if (!isActivationKey(event)) return;
						event.preventDefault();
						selectResource();
					}}
					tabIndex={0}
					role="button"
					aria-selected={isSelected}
				>
					{row.getVisibleCells().map((cell) => (
						<TableCell key={cell.id}>
							{flexRender(cell.column.columnDef.cell, cell.getContext())}
						</TableCell>
					))}
				</TableRow>
			)}
		</Fragment>
	);
}

function ResourceGroupHeader({
	label,
	count,
	collapsed,
	onToggle,
}: {
	label: string;
	count: number;
	collapsed: boolean;
	onToggle: () => void;
}) {
	const visual = getResourceGroupVisual(label);
	const Icon = visual.icon;

	return (
		<TableRow className="bg-muted/50 text-xs font-bold text-primary hover:bg-muted/50">
			<TableCell colSpan={columns.length} className="!p-0">
				<button
					type="button"
					className="flex w-full cursor-pointer items-center gap-2 border-0 bg-muted/50 px-3 py-2 text-left text-inherit focus-visible:ring-1 focus-visible:ring-ring/50"
					onClick={onToggle}
					aria-expanded={!collapsed}
				>
					{collapsed ? (
						<ChevronRight className="size-4 shrink-0 text-muted-foreground" />
					) : (
						<ChevronDown className="size-4 shrink-0 text-muted-foreground" />
					)}
					<Icon className={cn("size-3.5 shrink-0", visual.className)} />
					<span className="ml-0 text-muted-foreground">{label}</span>
					<small className="ml-0.5 text-[0.6875rem] font-medium text-muted-foreground">
						{count} resources on this page
					</small>
				</button>
			</TableCell>
		</TableRow>
	);
}

function ResourceTypeGroupHeader({
	label,
	kind,
	count,
	collapsed,
	onToggle,
}: {
	label: string;
	kind: string;
	count: number;
	collapsed: boolean;
	onToggle: () => void;
}) {
	const visual = getResourceKindVisual(kind);
	const Icon = visual.icon;

	return (
		<TableRow className="bg-card text-[0.72rem] font-bold uppercase text-foreground hover:bg-card">
			<TableCell colSpan={columns.length} className="!p-0">
				<button
					type="button"
					className="flex w-full cursor-pointer items-center gap-2 border-0 bg-card py-1.5 pl-6 pr-3 text-left text-[0.6875rem] text-inherit focus-visible:ring-1 focus-visible:ring-ring/50"
					onClick={onToggle}
					aria-expanded={!collapsed}
				>
					{collapsed ? (
						<ChevronRight className="size-4 shrink-0 text-muted-foreground" />
					) : (
						<ChevronDown className="size-4 shrink-0 text-muted-foreground" />
					)}
					<Icon className={cn("size-3.5 shrink-0", visual.className)} />
					<span className="ml-0 text-inherit">{label}</span>
					<small className="ml-0.5 text-[0.625rem] font-medium normal-case text-muted-foreground">
						{count} on this page
					</small>
				</button>
			</TableCell>
		</TableRow>
	);
}

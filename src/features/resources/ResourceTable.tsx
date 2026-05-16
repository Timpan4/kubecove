import { Fragment } from "react";
import {
	flexRender,
	type Row,
	type Table,
} from "@tanstack/react-table";
import { ChevronDown, ChevronRight } from "lucide-react";
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
	resourceGroupCollapseKey,
	resourceTypeGroupCollapseKey,
} from "./helpers";

interface ResourceTableProps {
	table: Table<ResourceSummary>;
	groupedByArgo: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
	onToggleGroup: (key: string) => void;
	onSelectedResourceKeyChange: (key: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function resourceRowKey(resource: ResourceSummary): string {
	return `${resource.cluster}:${resource.kind}:${resource.namespace ?? ""}:${resource.name}`;
}

export function ResourceTable({
	table,
	groupedByArgo,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
}: ResourceTableProps) {
	const rowModel = table.getRowModel();

	return (
		<div className="w-full max-w-full min-w-0 overflow-x-auto">
			<table className={TABLE_CLASS}>
				<thead>
					{table.getHeaderGroups().map((headerGroup) => (
						<tr key={headerGroup.id}>
							{headerGroup.headers.map((header) => (
								<th
									key={header.id}
									onClick={header.column.getToggleSortingHandler()}
									className={
										header.column.getCanSort()
											? "cursor-pointer"
											: "cursor-default"
									}
								>
									{flexRender(
										header.column.columnDef.header,
										header.getContext(),
									)}
									{header.column.getIsSorted() === "asc"
										? " ↑"
										: header.column.getIsSorted() === "desc"
											? " ↓"
											: ""}
								</th>
							))}
						</tr>
					))}
				</thead>
				<tbody>
					{rowModel.rows.length === 0 ? (
						<tr>
							<td colSpan={columns.length} className={EMPTY_PAGE_CLASS}>
								No resources match your filter
							</td>
						</tr>
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
								onToggleGroup={onToggleGroup}
								onSelectedResourceKeyChange={onSelectedResourceKeyChange}
								onResourceSelect={onResourceSelect}
							/>
						))
					)}
				</tbody>
			</table>
		</div>
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
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
}: ResourceTableRowProps) {
	const resourceKey = resourceRowKey(row.original);
	const isSelected = selectedResourceKey === resourceKey;
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
				<tr
					className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
					onClick={() => {
						diagnosticLog("resources.row.click", {
							key: resourceKey,
							alreadySelected: isSelected,
							rowIndex: index,
						});
						onSelectedResourceKeyChange(resourceKey);
						onResourceSelect(row.original);
					}}
				>
					{row.getVisibleCells().map((cell) => (
						<td key={cell.id}>
							{flexRender(cell.column.columnDef.cell, cell.getContext())}
						</td>
					))}
				</tr>
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
		<tr className="[&_td]:bg-muted/50 [&_td]:p-0 [&_td]:text-xs [&_td]:font-bold [&_td]:text-primary">
			<td colSpan={columns.length} className="!p-0">
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
			</td>
		</tr>
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
		<tr className="[&_td]:bg-card [&_td]:p-0 [&_td]:text-[0.72rem] [&_td]:font-bold [&_td]:uppercase [&_td]:text-foreground">
			<td colSpan={columns.length} className="!p-0">
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
			</td>
		</tr>
	);
}

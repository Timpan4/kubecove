import { Fragment, type KeyboardEvent, useEffect, useRef } from "react";
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
import {
	EMPTY_PAGE_CLASS,
	ROW_CLASS,
	SELECTED_ROW_CLASS,
	STICKY_APP_GROUP_TOP,
	STICKY_TYPE_GROUP_TOP,
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
	groupedByGitOps: boolean;
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

function scrollSelectedRowIntoView(viewport: HTMLDivElement | null) {
	if (!viewport) return;
	const selectedRow = viewport.querySelector<HTMLElement>(
		"tr[data-resource-selected='true']",
	);
	if (!selectedRow) return;
	selectedRow.scrollIntoView({ block: "center", inline: "nearest" });
}

export function ResourceTable({
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
}: ResourceTableProps) {
	const tableViewportRef = useRef<HTMLDivElement | null>(null);
	const rowModel = table.getRowModel();
	const hasExactSelectedResource =
		selectedResourceKey !== null &&
		rowModel.rows.some(
			(row) => resourceSelectionKey(row.original) === selectedResourceKey,
		);

	useEffect(() => {
		const viewport = tableViewportRef.current;
		if (!viewport || (!selectedResourceKey && !selectedResourceIdentityKey)) return;
		let secondFrame: number | null = null;
		const firstFrame = window.requestAnimationFrame(() => {
			scrollSelectedRowIntoView(viewport);
			secondFrame = window.requestAnimationFrame(() => {
				scrollSelectedRowIntoView(viewport);
			});
		});
		return () => {
			window.cancelAnimationFrame(firstFrame);
			if (secondFrame !== null) window.cancelAnimationFrame(secondFrame);
		};
	}, [
		collapsedGroups,
		rowModel.rows,
		selectedResourceIdentityKey,
		selectedResourceKey,
	]);

	useEffect(() => {
		const viewport = tableViewportRef.current;
		if (!viewport || (!selectedResourceKey && !selectedResourceIdentityKey)) return;
		const resizeObserver = new ResizeObserver(() => {
			scrollSelectedRowIntoView(viewport);
		});
		resizeObserver.observe(viewport);
		return () => resizeObserver.disconnect();
	}, [selectedResourceIdentityKey, selectedResourceKey]);

	// Measure the sticky levels and expose their offsets as CSS variables, so
	// the group rows pin exactly under the header (with a 1px tuck to avoid
	// seams) regardless of padding/zoom/font changes.
	useEffect(() => {
		const viewport = tableViewportRef.current;
		if (!viewport) return;
		const measure = () => {
			const headerRow = viewport.querySelector("thead tr");
			const appGroupCell = viewport.querySelector('[data-sticky="app-group"]');
			const headerHeight = headerRow
				? Math.round(headerRow.getBoundingClientRect().height)
				: 0;
			const appGroupHeight = appGroupCell
				? Math.round(appGroupCell.getBoundingClientRect().height)
				: 0;
			viewport.style.setProperty(
				"--sticky-app-top",
				`${Math.max(headerHeight - 1, 0)}px`,
			);
			viewport.style.setProperty(
				"--sticky-type-top",
				`${Math.max(headerHeight + appGroupHeight - 2, 0)}px`,
			);
		};
		measure();
		const resizeObserver = new ResizeObserver(measure);
		resizeObserver.observe(viewport);
		return () => resizeObserver.disconnect();
	}, []);

	// colSpan must match the rendered column count exactly: a colSpan larger
	// than the real number of columns makes the browser synthesize phantom
	// columns, which widens the table past its last column.
	const visibleColumnCount = table.getVisibleLeafColumns().length;

	return (
		<div
			ref={tableViewportRef}
			// The ui Table wraps the <table> in an overflow-x-auto container,
			// which would swallow position:sticky — neutralize it so the sticky
			// header/group rows pin to this scrolling viewport instead.
			className="scrollbar-classic h-full min-h-0 overflow-auto [&_[data-slot=table-container]]:overflow-visible"
		>
			{/* Width = sum of column sizes (so the scroll range ends exactly at
			    the last column); when the panel is wider, the name column has
			    no fixed width and absorbs the remaining space. */}
			<Table
				className={TABLE_CLASS}
				style={{ minWidth: table.getCenterTotalSize() }}
			>
				<colgroup>
					{table.getVisibleLeafColumns().map((column) => (
						<col
							key={column.id}
							style={
								column.id === "name"
									? undefined
									: { width: `${column.getSize()}px` }
							}
						/>
					))}
				</colgroup>
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
							<TableCell
								colSpan={visibleColumnCount}
								className={EMPTY_PAGE_CLASS}
							>
								No resources match your filter
							</TableCell>
						</TableRow>
					) : (
						rowModel.rows.map((row, index) => (
							<ResourceTableRow
								key={row.id}
								row={row}
								index={index}
								visibleColumnCount={visibleColumnCount}
								previous={index > 0 ? rowModel.rows[index - 1]?.original : null}
								groupedByGitOps={groupedByGitOps}
								pageGroups={pageGroups}
								pageTypeGroups={pageTypeGroups}
								collapsedGroups={collapsedGroups}
								selectedResourceKey={selectedResourceKey}
								selectedResourceIdentityKey={selectedResourceIdentityKey}
								hasExactSelectedResource={hasExactSelectedResource}
								onToggleGroup={onToggleGroup}
								onSelectedResourceKeyChange={onSelectedResourceKeyChange}
								onResourceSelect={onResourceSelect}
							/>
						))
					)}
				</TableBody>
			</Table>
		</div>
	);
}

interface ResourceTableRowProps {
	row: Row<ResourceSummary>;
	index: number;
	visibleColumnCount: number;
	previous: ResourceSummary | null;
	groupedByGitOps: boolean;
	pageGroups: Map<string, number>;
	pageTypeGroups: Map<string, number>;
	collapsedGroups: Set<string>;
	selectedResourceKey: string | null;
	selectedResourceIdentityKey: string | null;
	hasExactSelectedResource: boolean;
	onToggleGroup: (key: string) => void;
	onSelectedResourceKeyChange: (key: string) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
}

function ResourceTableRow({
	row,
	index,
	visibleColumnCount,
	previous,
	groupedByGitOps,
	pageGroups,
	pageTypeGroups,
	collapsedGroups,
	selectedResourceKey,
	selectedResourceIdentityKey,
	hasExactSelectedResource,
	onToggleGroup,
	onSelectedResourceKeyChange,
	onResourceSelect,
}: ResourceTableRowProps) {
	const resourceKey = resourceSelectionKey(row.original);
	const identityKey = resourceIdentityKey(row.original);
	const isSelected =
		selectedResourceKey === resourceKey ||
		(!hasExactSelectedResource && selectedResourceIdentityKey === identityKey);
	const label = formatResourceGroupLabel(row.original);
	const typeLabel = formatResourceTypeGroupLabel(row.original);
	const typeKey = `${label}::${typeLabel}`;
	const groupCollapseKey = resourceGroupCollapseKey(row.original);
	const typeCollapseKey = resourceTypeGroupCollapseKey(row.original);
	const groupCollapsed = collapsedGroups.has(groupCollapseKey);
	const typeCollapsed = collapsedGroups.has(typeCollapseKey);
	const showGroupHeader =
		groupedByGitOps &&
		(!previous || formatResourceGroupLabel(previous) !== label);
	const showTypeGroupHeader =
		groupedByGitOps &&
		(!previous ||
			formatResourceGroupLabel(previous) !== label ||
			formatResourceTypeGroupLabel(previous) !== typeLabel);
	const hideResourceRow = groupedByGitOps && (groupCollapsed || typeCollapsed);
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
					colSpan={visibleColumnCount}
					collapsed={groupCollapsed}
					onToggle={() => onToggleGroup(groupCollapseKey)}
				/>
			)}
			{showTypeGroupHeader && !groupCollapsed && (
				<ResourceTypeGroupHeader
					label={typeLabel}
					kind={row.original.kind}
					count={pageTypeGroups.get(typeKey) ?? 0}
					colSpan={visibleColumnCount}
					collapsed={typeCollapsed}
					onToggle={() => onToggleGroup(typeCollapseKey)}
				/>
			)}
			{!hideResourceRow && (
				<TableRow
					data-resource-selected={isSelected ? "true" : undefined}
					className={cn(ROW_CLASS, isSelected && SELECTED_ROW_CLASS)}
					onClick={selectResource}
					onKeyDown={(event) => {
						if (!isActivationKey(event)) return;
						event.preventDefault();
						selectResource();
					}}
					tabIndex={0}
					role="button"
					aria-pressed={isSelected}
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
	colSpan,
	collapsed,
	onToggle,
}: {
	label: string;
	count: number;
	colSpan: number;
	collapsed: boolean;
	onToggle: () => void;
}) {
	const visual = getResourceGroupVisual(label);
	const Icon = visual.icon;

	return (
		<TableRow className="text-xs font-bold text-primary hover:bg-transparent">
			<TableCell
				colSpan={colSpan}
				data-sticky="app-group"
				className={cn("sticky z-20 !p-0 bg-background", STICKY_APP_GROUP_TOP)}
			>
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
	colSpan,
	collapsed,
	onToggle,
}: {
	label: string;
	kind: string;
	count: number;
	colSpan: number;
	collapsed: boolean;
	onToggle: () => void;
}) {
	const visual = getResourceKindVisual(kind);
	const Icon = visual.icon;

	return (
		<TableRow className="text-[0.72rem] font-bold uppercase text-foreground hover:bg-transparent">
			<TableCell
				colSpan={colSpan}
				className={cn("sticky z-10 !p-0 bg-card", STICKY_TYPE_GROUP_TOP)}
			>
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

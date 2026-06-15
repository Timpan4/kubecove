import type {
	SortingState,
	TableState,
	VisibilityState,
} from "@tanstack/react-table";

const RESOURCE_TABLE_DEFAULT_STATE: TableState = {
	columnSizing: {},
	columnSizingInfo: {
		columnSizingStart: [],
		deltaOffset: null,
		deltaPercentage: null,
		isResizingColumn: false,
		startOffset: null,
		startSize: null,
	},
	rowSelection: {},
	rowPinning: { top: [], bottom: [] },
	expanded: {},
	grouping: [],
	sorting: [],
	columnFilters: [],
	globalFilter: undefined,
	columnPinning: { left: [], right: [] },
	columnOrder: [],
	columnVisibility: {},
	pagination: { pageIndex: 0, pageSize: 10 },
};

export function createResourceTableState(
	sorting: SortingState,
	columnVisibility: VisibilityState,
): TableState {
	return {
		...RESOURCE_TABLE_DEFAULT_STATE,
		sorting,
		columnVisibility,
	};
}

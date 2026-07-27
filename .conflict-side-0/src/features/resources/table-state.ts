export type ResourceSortingState = Array<{ id: string; desc: boolean }>;
export type ResourceColumnVisibilityState = Record<string, boolean>;

export interface ResourceTableState {
	columnSizing: Record<string, number>;
	columnSizingInfo: {
		columnSizingStart: [string, number][];
		deltaOffset: number | null;
		deltaPercentage: number | null;
		isResizingColumn: false | string;
		startOffset: number | null;
		startSize: number | null;
	};
	rowSelection: Record<string, boolean>;
	rowPinning: { top: string[]; bottom: string[] };
	expanded: Record<string, boolean>;
	grouping: string[];
	sorting: ResourceSortingState;
	columnFilters: Array<{ id: string; value: unknown }>;
	globalFilter: unknown;
	columnPinning: { left: string[]; right: string[] };
	columnOrder: string[];
	columnVisibility: ResourceColumnVisibilityState;
	pagination: { pageIndex: number; pageSize: number };
}

const RESOURCE_TABLE_DEFAULT_STATE: ResourceTableState = {
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
	sorting: ResourceSortingState,
	columnVisibility: ResourceColumnVisibilityState,
): ResourceTableState {
	return {
		...RESOURCE_TABLE_DEFAULT_STATE,
		sorting,
		columnVisibility,
	};
}

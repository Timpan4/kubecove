import { useState, type Dispatch, type SetStateAction } from "react";
import {
	createTable,
	getCoreRowModel,
	type SortingState,
	type VisibilityState,
} from "@tanstack/react-table";
import type { ResourceSummary } from "@/lib/types";
import { columns } from "./columns";
import {
	ResourceMapTableLayout,
	type ResourceMapTableLayoutProps,
} from "./ResourceMapTableLayout";
import { createResourceTableState } from "./table-state";

type ResourceTableLayoutBoundaryProps = Omit<
	ResourceMapTableLayoutProps,
	"table"
> & {
	pageRows: ResourceSummary[];
	sorting: SortingState;
	columnVisibility: VisibilityState;
	onSortingChange: Dispatch<SetStateAction<SortingState>>;
};

export function ResourceTableLayoutBoundary({
	pageRows,
	sorting,
	columnVisibility,
	onSortingChange,
	...layoutProps
}: ResourceTableLayoutBoundaryProps) {
	const tableOptions = {
		data: pageRows,
		columns,
		state: createResourceTableState(sorting, columnVisibility),
		onSortingChange,
		getCoreRowModel: getCoreRowModel(),
		onStateChange: () => {},
		renderFallbackValue: null,
	};
	const [table] = useState(() => createTable<ResourceSummary>(tableOptions));

	table.setOptions((previous) => ({
		...previous,
		...tableOptions,
	}));

	return <ResourceMapTableLayout {...layoutProps} table={table} />;
}

import type { GitOpsOwnershipFilter } from "@/lib/gitops-ownership-evidence";
import type { ResourceSummary } from "@/lib/types";
import type { HealthFilter, HealthSummary, ResourceSearchEntry } from "./helpers";

export type ResourceSortColumn =
	| "name"
	| "namespace"
	| "kind"
	| "status"
	| "ready"
	| "restarts"
	| "age"
	| "cpu"
	| "memory";

export interface ResourceSort {
	id: ResourceSortColumn;
	desc: boolean;
}

export interface ResourceTableState {
	search: string;
	gitOpsFilter: string;
	healthFilter: HealthFilter;
	sort: ResourceSort;
	pageIndex: number;
	collapsedGroups: Set<string>;
	selectedResource?: ResourceSummary | null;
	preferredGitOpsResourceKeys?: ReadonlySet<string>;
}

export type ResourceTableEntry =
	| {
			type: "group";
			key: string;
			label: string;
			count: number;
			collapsed: boolean;
	  }
	| {
			type: "type";
			key: string;
			label: string;
			kind: string;
			count: number;
			collapsed: boolean;
	  }
	| {
			type: "resource";
			key: string;
			resource: ResourceSummary;
	  };

export interface ResourceTableProjection {
	searchIndex: ResourceSearchEntry[];
	gitOpsFilters: GitOpsOwnershipFilter[];
}

export interface ResourceTableModel {
	scopedRows: ResourceSummary[];
	filteredRows: ResourceSummary[];
	displayRows: ResourceSummary[];
	pageRows: ResourceSummary[];
	entries: ResourceTableEntry[];
	totalRows: number;
	pageCount: number;
	safePageIndex: number;
	groupedByGitOps: boolean;
	gitOpsFilters: GitOpsOwnershipFilter[];
	healthSummary: HealthSummary;
	columnVisibility: {
		ready: boolean;
		restarts: boolean;
		cpu: boolean;
		memory: boolean;
		gitOps: boolean;
	};
}

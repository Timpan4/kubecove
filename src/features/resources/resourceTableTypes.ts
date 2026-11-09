import type { GitOpsOwnershipFilter } from "@/lib/gitops-ownership-evidence";
import type { ResourceSummary } from "@/lib/types";
import type {
	HealthFilter,
	HealthSummary,
	ResourceSearchEntry,
} from "./helpers";

export type ResourceSortColumn =
	| "name"
	| "context"
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
	kindOrder?: readonly string[];
	/** Workspace contexts in rank order. Ties under any sort column order by this rank. */
	contexts?: readonly string[];
	/** Contexts whose rows show. Empty or absent shows every context. */
	contextFilter?: ReadonlySet<string>;
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
	/** The workspace has more than one context: owner and Helm groups and owner filters are per context. */
	multiContext: boolean;
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
	multiContext: boolean;
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

export type ResourceTableDisplay = Pick<
	ResourceTableModel,
	| "scopedRows"
	| "filteredRows"
	| "displayRows"
	| "totalRows"
	| "pageCount"
	| "groupedByGitOps"
	| "multiContext"
	| "gitOpsFilters"
	| "healthSummary"
> & {
	rowForResource: (resource: ResourceSummary) => ResourceSummary | undefined;
};

export type ResourceTablePage = Omit<ResourceTableModel, "entries"> &
	Pick<ResourceTableDisplay, "rowForResource">;

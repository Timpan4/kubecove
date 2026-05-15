import { Button } from "@/components/ui/button";

const PAGINATION_CLASS =
	"flex items-center justify-between border-t py-2 text-xs text-muted-foreground";

interface ResourcePaginationProps {
	totalRows: number;
	search: string;
	pageIndex: number;
	pageCount: number;
	onPageChange: (updater: (page: number) => number) => void;
}

export function ResourcePagination({
	totalRows,
	search,
	pageIndex,
	pageCount,
	onPageChange,
}: ResourcePaginationProps) {
	return (
		<div className={PAGINATION_CLASS}>
			<Button
				type="button"
				variant="outline"
				size="sm"
				onClick={() => onPageChange((page) => Math.max(0, page - 1))}
				disabled={pageIndex === 0}
			>
				Previous
			</Button>
			<span>
				{totalRows} {search ? "filtered" : "total"} rows
			</span>
			<span>
				Page {pageIndex + 1} of {pageCount}
			</span>
			<Button
				type="button"
				variant="outline"
				size="sm"
				onClick={() =>
					onPageChange((page) => Math.min(pageCount - 1, page + 1))
				}
				disabled={pageIndex >= pageCount - 1}
			>
				Next
			</Button>
		</div>
	);
}

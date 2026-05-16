import { Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";

const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";

interface ResourceToolbarProps {
	search: string;
	argoApps: string[];
	selectedArgoAppFilter: string;
	hasNoFilterResults: boolean;
	onSearchChange: (search: string) => void;
	onArgoAppFilterChange: (app: string) => void;
	onClearFilters: () => void;
}

export function ResourceToolbar({
	search,
	argoApps,
	selectedArgoAppFilter,
	hasNoFilterResults,
	onSearchChange,
	onArgoAppFilterChange,
	onClearFilters,
}: ResourceToolbarProps) {
	const hasFilters = Boolean(search || selectedArgoAppFilter);

	return (
		<div className={TOOLBAR_CLASS}>
			<div className="relative min-w-0 flex-1">
				<Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
				<Input
					aria-label="Search resources by name, namespace, kind, owner, Argo app, or Helm release"
					className="h-9 border-slate-700/80 bg-slate-950/45 pl-8 text-sm text-foreground placeholder:text-muted-foreground"
					type="text"
					placeholder="Search by name, namespace, kind, owner, Argo app, Helm release..."
					value={search}
					onChange={(e) => onSearchChange(e.target.value)}
				/>
			</div>
			{argoApps.length > 0 && (
				<Select
					value={selectedArgoAppFilter || "all"}
					onValueChange={(value) =>
						onArgoAppFilterChange(value === "all" ? "" : value)
					}
				>
					<SelectTrigger
						className="h-9 max-w-52 border-slate-700/80 bg-slate-950/45 text-foreground"
						aria-label="Filter by Argo application"
					>
						<SelectValue placeholder="All Argo apps" />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value="all">All Argo apps</SelectItem>
						{argoApps.map((app) => (
							<SelectItem key={app} value={app}>
								{app}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			)}
			{hasFilters && (
				<Button
					type="button"
					variant="outline"
					size="sm"
					onClick={onClearFilters}
				>
					<X className="size-3.5" />
					Clear
				</Button>
			)}
			{hasFilters && hasNoFilterResults && (
				<span className="text-xs text-muted-foreground">
					No results for current filters{" "}
					<Button
						type="button"
						variant="link"
						size="sm"
						className="h-auto px-0 py-0 text-xs"
						onClick={onClearFilters}
					>
						clear filters
					</Button>
				</span>
			)}
		</div>
	);
}

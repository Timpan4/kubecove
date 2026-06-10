import { Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	InputGroup,
	InputGroupAddon,
	InputGroupInput,
	InputGroupText,
} from "@/components/ui/input-group";
import {
	Select,
	SelectContent,
	SelectGroup,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import type { GitOpsFilterOption } from "./helpers";

const TOOLBAR_CLASS = "mb-1 flex items-center gap-2 p-0";

interface ResourceToolbarProps {
	search: string;
	gitOpsFilters: GitOpsFilterOption[];
	selectedArgoAppFilter: string;
	hasNoFilterResults: boolean;
	onSearchChange: (search: string) => void;
	onArgoAppFilterChange: (app: string) => void;
	onClearFilters: () => void;
}

export function ResourceToolbar({
	search,
	gitOpsFilters,
	selectedArgoAppFilter,
	hasNoFilterResults,
	onSearchChange,
	onArgoAppFilterChange,
	onClearFilters,
}: ResourceToolbarProps) {
	const hasFilters = Boolean(search || selectedArgoAppFilter);

	return (
		<div className={TOOLBAR_CLASS}>
			<InputGroup className="h-9 min-w-0 flex-1 border-slate-700/80 bg-slate-950/45">
				<InputGroupAddon align="inline-start">
					<InputGroupText>
						<Search className="size-4" />
					</InputGroupText>
				</InputGroupAddon>
				<InputGroupInput
					aria-label="Search resources by name, namespace, kind, owner, GitOps owner, or Helm release"
					className="h-8 text-sm text-foreground placeholder:text-muted-foreground"
					type="text"
					placeholder="Search by name, namespace, kind, owner, GitOps owner, Helm release..."
					value={search}
					onChange={(e) => onSearchChange(e.target.value)}
				/>
			</InputGroup>
			{gitOpsFilters.length > 0 && (
				<Select
					value={selectedArgoAppFilter || "all"}
					onValueChange={(value) =>
						onArgoAppFilterChange(value === "all" ? "" : value)
					}
				>
					<SelectTrigger
						className="h-9 max-w-52 border-slate-700/80 bg-slate-950/45 text-foreground"
						aria-label="Filter by GitOps owner"
					>
						<SelectValue placeholder="All GitOps owners" />
					</SelectTrigger>
					<SelectContent>
						<SelectGroup>
							<SelectItem value="all">All GitOps owners</SelectItem>
							{gitOpsFilters.map((filter) => (
								<SelectItem key={filter.key} value={filter.key}>
									{filter.label}
								</SelectItem>
							))}
						</SelectGroup>
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

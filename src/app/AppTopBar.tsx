import { FolderOpen, Search, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ClusterSelector } from "@/components/ClusterSelector";
import { UpdateStatusButton } from "@/features/app-updates";
import { ActivePortForwards } from "@/features/live-sessions";

interface AppTopBarProps {
	clusterContext: string;
	contentTitle: string;
	onClusterChange: (cluster: string) => void;
	onOpenLauncher: () => void;
	onOpenSettings: () => void;
	showClusterSelector?: boolean;
	showSearch?: boolean;
}

export function AppTopBar({
	clusterContext,
	contentTitle,
	onClusterChange,
	onOpenLauncher,
	onOpenSettings,
	showClusterSelector = true,
	showSearch = true,
}: AppTopBarProps) {
	return (
		<header className="flex h-12 shrink-0 items-center gap-4 border-b bg-sidebar px-4 [-webkit-app-region:drag]">
			<div className="flex shrink-0 items-center gap-3 [-webkit-app-region:no-drag]">
				{showClusterSelector && (
					<ClusterSelector
						value={clusterContext}
						onClusterChange={onClusterChange}
					/>
				)}
			</div>
			<div className="flex min-w-0 flex-1 items-center justify-center">
				<span className="truncate whitespace-nowrap text-sm font-semibold">
					{contentTitle}
				</span>
			</div>
			<div className="flex shrink-0 items-center">
				<ActivePortForwards />
				<UpdateStatusButton />
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="mr-1 size-8 text-muted-foreground [-webkit-app-region:no-drag]"
					aria-label="Open workspaces"
					onClick={onOpenLauncher}
				>
					<FolderOpen />
				</Button>
				<Button
					type="button"
					variant="ghost"
					size="icon"
					className="mr-2 size-8 text-muted-foreground [-webkit-app-region:no-drag]"
					aria-label="Open settings"
					onClick={onOpenSettings}
				>
					<Settings />
				</Button>
				{showSearch && (
					<div className="flex items-center gap-2 whitespace-nowrap rounded-md border bg-background/50 px-3 py-1.5 text-xs text-muted-foreground">
						<Search className="size-3.5" aria-hidden="true" />
						<span>Search resources...</span>
					</div>
				)}
			</div>
		</header>
	);
}

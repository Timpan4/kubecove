import { FolderOpen, Search, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ClusterSelector } from "@/components/ClusterSelector";
import { UpdateStatusButton } from "@/features/app-updates";
import { useCommandPaletteStore } from "@/features/command-palette";
import { ActivePortForwards } from "@/features/live-sessions/ActivePortForwards";
import { RuntimeBadge } from "./runtime/RuntimeBadge";

const IS_MAC =
	typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);
const SEARCH_SHORTCUT_HINT = IS_MAC ? "⌘K" : "Ctrl K";

interface AppTopBarProps {
	clusterContext: string;
	contentTitle: string;
	onClusterChange: (cluster: string) => void;
	onOpenLauncher: () => void;
	onOpenSettings: () => void;
	onOpenPortForwards?: () => void;
	showClusterSelector?: boolean;
	showSearch?: boolean;
}

export function AppTopBar({
	clusterContext,
	contentTitle,
	onClusterChange,
	onOpenLauncher,
	onOpenSettings,
	onOpenPortForwards,
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
				<ActivePortForwards onOpenManager={onOpenPortForwards} />
				<UpdateStatusButton />
				<RuntimeBadge onOpenSettings={onOpenSettings} />
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
					<button
						type="button"
						className="flex cursor-pointer items-center gap-2 whitespace-nowrap rounded-md border border-border/60 bg-surface-1 px-3 py-1.5 text-xs text-muted-foreground shadow-xs transition-all [-webkit-app-region:no-drag] hover:bg-surface-2 hover:text-foreground hover:shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/30"
						aria-label="Search views, namespaces, and resources"
						onClick={() => useCommandPaletteStore.getState().setOpen(true)}
					>
						<Search className="size-3.5" aria-hidden="true" />
						<span>Search resources...</span>
						<kbd className="rounded border bg-muted px-1 py-px font-mono text-xs text-muted-foreground">
							{SEARCH_SHORTCUT_HINT}
						</kbd>
					</button>
				)}
			</div>
		</header>
	);
}

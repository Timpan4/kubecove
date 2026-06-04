import { Suspense } from "react";
import { AppTopBar } from "./AppTopBar";
import { AppUsageFooter } from "./AppUsageFooter";
import { SettingsPage, WorkspaceLauncher } from "./lazyViews";
import { ViewLoadingFallback } from "./ViewLoadingFallback";
import type { SavedWorkspace } from "@/lib/workspaces";

export function LauncherShell({
	clusterContext,
	viewMode,
	showUsageFooter,
	onClusterChange,
	onOpenLauncher,
	onOpenSettings,
	onOpenWorkspace,
}: {
	clusterContext: string;
	viewMode: string;
	showUsageFooter: boolean;
	onClusterChange: (cluster: string) => void;
	onOpenLauncher: () => void;
	onOpenSettings: () => void;
	onOpenWorkspace: (workspace: SavedWorkspace) => void;
}) {
	return (
		<div className="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<AppTopBar
				clusterContext={clusterContext}
				contentTitle={viewMode === "settings" ? "Settings" : "Workspaces"}
				onClusterChange={onClusterChange}
				onOpenLauncher={onOpenLauncher}
				onOpenSettings={onOpenSettings}
				showClusterSelector={false}
				showSearch={false}
			/>
			<div className="min-h-0 flex-1 overflow-hidden">
				{viewMode === "settings" ? (
					<div className="h-full overflow-y-auto overflow-x-hidden p-4 md:px-6">
						<Suspense fallback={<ViewLoadingFallback label="Loading settings..." />}>
							<SettingsPage />
						</Suspense>
					</div>
				) : (
					<Suspense fallback={<ViewLoadingFallback label="Loading workspaces..." />}>
						<WorkspaceLauncher onOpenWorkspace={onOpenWorkspace} />
					</Suspense>
				)}
			</div>
			<AppUsageFooter visible={showUsageFooter} />
		</div>
	);
}

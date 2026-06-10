import { Suspense } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { SavedPortForwardRestorePrompt } from "@/features/live-sessions/SavedPortForwardRestorePrompt";
import {
	argoApplicationGitOpsFilterKey,
	type HealthFilter,
} from "@/features/resources/helpers";
import type { ResourceGitOpsFocus } from "@/features/resources/ResourceGitOpsFocusSummary";
import type { ArgoSelectedItem, DashboardViewMode } from "@/lib/hooks";
import type {
	ArgoApplicationSummary,
	FluxResourceSummary,
	HelmReleaseSummary,
	ResourceKindSelection,
	ResourceSummary,
} from "@/lib/types";
import type { TreeNodeId } from "@/lib/tree-nav";
import type { SavedWorkspace } from "@/lib/workspaces";
import {
	GitOpsPanel,
	HelmPanel,
	IncidentCockpit,
	RbacPanel,
	ResourceList,
	SettingsPage,
	WorkspaceOverview,
	WorkspacePortForwardsPage,
} from "./lazyViews";
import { ViewLoadingFallback } from "./ViewLoadingFallback";

interface AppMainContentProps {
	activeWorkspace: SavedWorkspace;
	viewMode: DashboardViewMode;
	liveSessionCleanupMessage: string | null;
	onDismissLiveSessionCleanup: () => void;
	showPortForwardRestorePrompt: boolean;
	onDismissPortForwardRestore: () => void;
	onReviewPortForwards: () => void;
	onOpenResources: (namespace?: string, healthFilter?: HealthFilter) => void;
	onOpenArgo: (argoApp?: string) => void;
	onOpenIncidents: () => void;
	onOpenPortForwards: () => void;
	onOpenLauncher: () => void;
	onBackFromSettings: () => void;
	clusterContext: string;
	selectedArgoApp: ArgoSelectedItem;
	onArgoItemSelect: (app: NonNullable<ArgoSelectedItem>) => void;
	onOpenArgoApplicationResources: (app: ArgoApplicationSummary) => void;
	selectedFluxResource: FluxResourceSummary | null;
	onFluxResourceSelect: (resource: FluxResourceSummary) => void;
	selectedTreeNode: TreeNodeId | null;
	selectedHelmRelease: HelmReleaseSummary | null;
	onHelmReleaseSelect: (release: HelmReleaseSummary) => void;
	targetHelmRelease: { name: string; namespace?: string } | null;
	onTargetHelmReleaseResolved: () => void;
	selectedNamespaces: string[];
	canQueryResources: boolean;
	computedNamespaces: string[];
	computedKinds: ResourceKindSelection[];
	selectedArgoAppFilter: string;
	selectedResource: ResourceSummary | null;
	resourceHealthFilter: HealthFilter;
	resourceInitialSearch: string;
	onArgoAppFilterChange: (app: string) => void;
	onNamespacesChange: (namespaces: string[]) => void;
	onKindsChange: (kinds: ResourceKindSelection[]) => void;
	onResourceSelect: (resource: ResourceSummary) => void;
	emptyMsg: string;
}

export function AppMainContent({
	activeWorkspace,
	viewMode,
	liveSessionCleanupMessage,
	onDismissLiveSessionCleanup,
	showPortForwardRestorePrompt,
	onDismissPortForwardRestore,
	onReviewPortForwards,
	onOpenResources,
	onOpenArgo,
	onOpenIncidents,
	onOpenPortForwards,
	onOpenLauncher,
	onBackFromSettings,
	clusterContext,
	selectedArgoApp,
	onArgoItemSelect,
	onOpenArgoApplicationResources,
	selectedFluxResource,
	onFluxResourceSelect,
	selectedTreeNode,
	selectedHelmRelease,
	onHelmReleaseSelect,
	targetHelmRelease,
	onTargetHelmReleaseResolved,
	selectedNamespaces,
	canQueryResources,
	computedNamespaces,
	computedKinds,
	selectedArgoAppFilter,
	selectedResource,
	resourceHealthFilter,
	resourceInitialSearch,
	onArgoAppFilterChange,
	onNamespacesChange,
	onKindsChange,
	onResourceSelect,
	emptyMsg,
}: AppMainContentProps) {
	const resourceGitOpsFocus =
		viewMode === "resources"
			? argoApplicationFocus(selectedArgoApp, selectedArgoAppFilter)
			: null;
	return (
		<main className="flex h-full w-full min-w-0 flex-col overflow-hidden">
			{liveSessionCleanupMessage && (
				<Alert className="rounded-none border-x-0 border-t-0">
					<AlertTitle>Live sessions updated</AlertTitle>
					<AlertDescription className="flex flex-wrap items-center justify-between gap-3">
						<span>{liveSessionCleanupMessage}</span>
						<button
							type="button"
							className="text-xs font-medium text-muted-foreground underline-offset-4 hover:underline"
							onClick={onDismissLiveSessionCleanup}
						>
							Dismiss
						</button>
					</AlertDescription>
				</Alert>
			)}
			{showPortForwardRestorePrompt && (
				<SavedPortForwardRestorePrompt
					workspace={activeWorkspace}
					onReview={onReviewPortForwards}
					onDismiss={onDismissPortForwardRestore}
				/>
			)}
			{viewMode === "overview" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading overview..." />}>
						<WorkspaceOverview
							workspace={activeWorkspace}
							onOpenResources={onOpenResources}
							onOpenArgo={onOpenArgo}
							onOpenIncidents={onOpenIncidents}
							onOpenPortForwards={onOpenPortForwards}
							onOpenLauncher={onOpenLauncher}
						/>
					</Suspense>
				</div>
			) : viewMode === "settings" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading settings..." />}>
						<SettingsPage onBack={onBackFromSettings} />
					</Suspense>
				</div>
			) : viewMode === "argo" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading GitOps..." />}>
						<GitOpsPanel
							clusterContext={clusterContext}
							selectedGitOpsItem={selectedArgoApp}
							onGitOpsItemSelect={onArgoItemSelect}
							onOpenArgoApplicationResources={onOpenArgoApplicationResources}
							selectedFluxResource={selectedFluxResource}
							onFluxResourceSelect={onFluxResourceSelect}
							selectedGitOpsKind={
								selectedTreeNode?.type === "kind" && selectedTreeNode.kind
									? selectedTreeNode.kind
									: null
							}
						/>
					</Suspense>
				</div>
			) : viewMode === "helm" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading Helm releases..." />}>
						<HelmPanel
							clusterContext={clusterContext}
							selectedRelease={selectedHelmRelease}
							onReleaseSelect={onHelmReleaseSelect}
							targetRelease={targetHelmRelease}
							onTargetReleaseResolved={onTargetHelmReleaseResolved}
						/>
					</Suspense>
				</div>
			) : viewMode === "incidents" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading incident cockpit..." />}>
						<IncidentCockpit
							workspace={activeWorkspace}
							onResourceSelect={onResourceSelect}
							onOpenResources={() => onOpenResources()}
						/>
					</Suspense>
				</div>
			) : viewMode === "portForwards" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading port forwards..." />}>
						<WorkspacePortForwardsPage workspace={activeWorkspace} />
					</Suspense>
				</div>
			) : viewMode === "rbac" ? (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					<Suspense fallback={<ViewLoadingFallback label="Loading RBAC inspection..." />}>
						<RbacPanel
							clusterContext={clusterContext}
							selectedNamespaces={selectedNamespaces}
							selectedView={
								selectedTreeNode?.type === "kind" && selectedTreeNode.kind
									? selectedTreeNode.kind
									: null
							}
						/>
					</Suspense>
				</div>
			) : (
				<div className="min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-6">
					{canQueryResources ? (
						<Suspense fallback={<ViewLoadingFallback label="Loading resources..." />}>
							<ResourceList
								clusterContext={clusterContext}
								selectedNamespaces={computedNamespaces}
								selectedKinds={computedKinds}
								selectedArgoAppFilter={selectedArgoAppFilter}
								gitOpsFocus={resourceGitOpsFocus}
								selectedResource={selectedResource}
								initialHealthFilter={resourceHealthFilter}
								initialSearch={resourceInitialSearch}
								onArgoAppFilterChange={onArgoAppFilterChange}
								onNamespacesChange={onNamespacesChange}
								onKindsChange={onKindsChange}
								onResourceSelect={onResourceSelect}
							/>
						</Suspense>
					) : (
						<div className="p-8 text-center text-sm text-muted-foreground">
							{emptyMsg}
						</div>
					)}
				</div>
			)}
		</main>
	);
}

function argoApplicationFocus(
	item: ArgoSelectedItem,
	filter: string,
): ResourceGitOpsFocus | null {
	if (!item) return null;
	if ("status" in item) return null;
	if (!("sourceRepo" in item) || !("destinationServer" in item)) return null;
	if (
		filter !== argoApplicationGitOpsFilterKey(item.name) &&
		filter !== item.name
	) {
		return null;
	}
	return { provider: "argo", application: item };
}

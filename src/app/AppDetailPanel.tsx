import { Suspense } from "react";
import {
	ArgoDetailPanel,
	HelmDetailPanel,
	ResourceDetailPanel,
} from "./lazyViews";
import { ViewLoadingFallback } from "./ViewLoadingFallback";
import type { DashboardViewMode, ArgoSelectedItem } from "@/lib/hooks";
import type { HelmReleaseSummary, ResourceSummary } from "@/lib/types";

export function AppDetailPanel({
	viewMode,
	selectedHelmRelease,
	selectedArgoApp,
	selectedResource,
	selectedResourceKey,
	onHelmClose,
	onArgoClose,
	onResourceClose,
	onOpenHelmResources,
	onOpenHelmReleaseFromResource,
}: {
	viewMode: DashboardViewMode;
	selectedHelmRelease: HelmReleaseSummary | null;
	selectedArgoApp: ArgoSelectedItem;
	selectedResource: ResourceSummary | null;
	selectedResourceKey: string | null;
	onHelmClose: () => void;
	onArgoClose: () => void;
	onResourceClose: () => void;
	onOpenHelmResources: (release: HelmReleaseSummary) => void;
	onOpenHelmReleaseFromResource: (
		releaseName: string,
		namespace?: string | null,
	) => void;
}) {
	if (viewMode === "helm" && selectedHelmRelease) {
		return (
			<Suspense fallback={<ViewLoadingFallback label="Loading Helm details..." />}>
				<HelmDetailPanel
					release={selectedHelmRelease}
					onClose={onHelmClose}
					onOpenResources={onOpenHelmResources}
				/>
			</Suspense>
		);
	}
	if (viewMode === "argo" && selectedArgoApp) {
		return (
			<Suspense fallback={<ViewLoadingFallback label="Loading app details..." />}>
				<ArgoDetailPanel app={selectedArgoApp} onClose={onArgoClose} />
			</Suspense>
		);
	}
	if (!selectedResource) return null;
	return (
		<Suspense fallback={<ViewLoadingFallback label="Loading resource details..." />}>
			<ResourceDetailPanel
				key={selectedResourceKey}
				resource={selectedResource}
				onClose={onResourceClose}
				onOpenHelmRelease={onOpenHelmReleaseFromResource}
			/>
		</Suspense>
	);
}

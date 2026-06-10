import { Suspense } from "react";
import {
	ArgoDetailPanel,
	FluxDetailPanel,
	HelmDetailPanel,
	ResourceDetailPanel,
} from "./lazyViews";
import { ViewLoadingFallback } from "./ViewLoadingFallback";
import type { DashboardViewMode, ArgoSelectedItem } from "@/lib/hooks";
import type { FluxResourceSummary, HelmReleaseSummary, ResourceSummary } from "@/lib/types";

export function AppDetailPanel({
	viewMode,
	selectedHelmRelease,
	selectedArgoApp,
	selectedFluxResource,
	selectedResource,
	selectedResourceKey,
	onHelmClose,
	onArgoClose,
	onFluxClose,
	onResourceClose,
	onOpenHelmResources,
	onOpenHelmReleaseFromResource,
}: {
	viewMode: DashboardViewMode;
	selectedHelmRelease: HelmReleaseSummary | null;
	selectedArgoApp: ArgoSelectedItem;
	selectedFluxResource: FluxResourceSummary | null;
	selectedResource: ResourceSummary | null;
	selectedResourceKey: string | null;
	onHelmClose: () => void;
	onArgoClose: () => void;
	onFluxClose: () => void;
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
	if (viewMode === "argo" && selectedFluxResource) {
		return (
			<Suspense fallback={<ViewLoadingFallback label="Loading Flux details..." />}>
				<FluxDetailPanel resource={selectedFluxResource} onClose={onFluxClose} />
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
